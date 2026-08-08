import { execFile } from 'node:child_process'
import { hostname } from 'node:os'
import { promisify } from 'node:util'
import { delay } from 'es-toolkit'
import ky from 'ky'
import { z } from 'zod'

// Push Codex (and residual Claude Code) usage to /api/usage/ingest via ccusage.
// Neither subscription exposes a token API, so the only source is each CLI's local
// transcripts. Those transcripts are consolidated onto ONE server across several
// same-user accounts that are mutually DISJOINT (each session lives in exactly one
// account). There is no automatic Mac->server sync, so the pusher first runs
// `usage-repartition.py route`: an ADDITIVE push of THIS Mac's sessions up to their
// topic account (no --delete). Then it runs `ccusage` REMOTELY on each account,
// pulls back only the small JSON rollup, and SUMS the rollups into one
// (date, provider, model) total that it POSTs. The ingest REPLACE-upserts each
// total in a durable workflow, so repeated runs converge with no double counting.
//
// Claude Code the product is retired on this Mac (2026-07), but the server may
// still hold historical ~/.claude transcripts; a missing/empty Claude report is
// skipped rather than aborting the host. Codex remains active (including via
// Cursor's Codex SDK). Cursor IDE spend is a SEPARATE writer
// (cho.sh/app/scripts/cursor-usage-push.ts: Team Admin + browser CSV) — ccusage
// has no cursor agent, so this pusher never writes provider:cursor.
//
// Config (machine-local environment; the host and account values are private and
// deliberately never committed):
//   CRON_SECRET         required: Bearer auth for the ingest endpoint
//   USAGE_COMPUTE_HOSTS required: comma-separated ssh accounts to compute on
//   USAGE_OPTIONAL_HOSTS optional: comma-separated hosts to SKIP (not abort) when
//                       unreachable (default none: every host is required)
//   USAGE_INGEST_URL    optional: defaults to https://cho.sh
//   USAGE_DRY_RUN       optional: "1" prints the merged rows and skips the POST

const CCUSAGE_VERSION = '20.0.19'
const INGEST_PATH = '/api/usage/ingest'
const DEFAULT_INGEST_URL = 'https://cho.sh'
const MAX_BUFFER = 256 * 1024 * 1024
const SSH_ARGS = ['-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10'] as const
// ccusage on the remote spawns `node` for its cli shim; each account symlinks
// ~/.local/bin/node -> bun, but a non-login ssh shell may not have it on PATH, so
// invoke bun's bunx by absolute path and put ~/.local/bin ahead for the node shim.
const REMOTE_ENV = 'PATH="$HOME/.local/bin:$PATH"'
const BUNX = '$HOME/.bun/bin/bunx'

const exec = promisify(execFile)
const http = ky.create({ retry: 0, throwHttpErrors: false, timeout: 30_000 })

const localHostnames = new Set(
  [hostname(), hostname().split('.')[0], 'localhost', 'local'].filter(Boolean),
)

const isLocalHost = (host: string): boolean => {
  if (host === 'local') {
    return true
  }
  const name = host.includes('@') ? host.split('@')[1] : host
  return localHostnames.has(name)
}

const runCcusage = async (
  host: string,
  tool: 'claude' | 'codex',
): Promise<{ stderr: string; stdout: string }> => {
  const remoteCmd =
    tool === 'claude'
      ? `${REMOTE_ENV} ${BUNX} ccusage@${CCUSAGE_VERSION} claude daily --json --breakdown`
      : `${REMOTE_ENV} ${BUNX} ccusage@${CCUSAGE_VERSION} codex daily --json`
  if (isLocalHost(host)) {
    const bunx = `${process.env.HOME}/.bun/bin/bunx`
    const args =
      tool === 'claude'
        ? [`ccusage@${CCUSAGE_VERSION}`, 'claude', 'daily', '--json', '--breakdown']
        : [`ccusage@${CCUSAGE_VERSION}`, 'codex', 'daily', '--json']
    return exec(bunx, args, {
      env: { ...process.env, PATH: `${process.env.HOME}/.local/bin:${process.env.PATH ?? ''}` },
      maxBuffer: MAX_BUFFER,
    })
  }
  return exec('ssh', [...SSH_ARGS, host, remoteCmd], { maxBuffer: MAX_BUFFER })
}

const claudeReport = z.object({
  daily: z
    .array(
      z.object({
        date: z.string(),
        modelBreakdowns: z
          .array(
            z.object({
              cacheCreationTokens: z.number().optional(),
              cacheReadTokens: z.number().optional(),
              cost: z.number().optional(),
              inputTokens: z.number().optional(),
              modelName: z.string(),
              outputTokens: z.number().optional(),
            }),
          )
          .optional(),
      }),
    )
    .optional(),
})

const codexReport = z.object({
  daily: z
    .array(
      z.object({
        costUSD: z.number().optional(),
        date: z.string(),
        models: z
          .record(
            z.string(),
            z.object({
              cacheCreationTokens: z.number().optional(),
              cacheReadTokens: z.number().optional(),
              inputTokens: z.number().optional(),
              outputTokens: z.number().optional(),
              totalTokens: z.number().optional(),
            }),
          )
          .optional(),
      }),
    )
    .optional(),
})

interface IngestRow {
  costUsd?: number
  date: string
  inputTokens: number
  model: string
  outputTokens: number
  provider: 'claude-code' | 'codex'
}

// One account's transcripts, deduped by ccusage, mapped to ingest rows. Runs on the
// remote over its OWN ~/.claude + ~/.codex only (0700-isolated from siblings).
// Codex failure aborts the host (required). Claude is best-effort: product retired
// on the Mac, so a missing report must not kill a Codex-bearing host.
const rowsForHost = async (host: string): Promise<IngestRow[]> => {
  const [claudeSettled, codexOut] = await Promise.all([
    runCcusage(host, 'claude').then(
      (out) => ({ ok: true as const, out }),
      (error: unknown) => ({ ok: false as const, error }),
    ),
    runCcusage(host, 'codex'),
  ])
  let claudeRows: IngestRow[] = []
  if (claudeSettled.ok) {
    claudeRows = (claudeReport.parse(JSON.parse(claudeSettled.out.stdout)).daily ?? []).flatMap((day) =>
      (day.modelBreakdowns ?? []).map((model) => ({
        // A missing or zero cost from ccusage means unpriced, not free: omit the field
        // so the ingest records "no cost data" instead of a false $0.
        ...(model.cost !== undefined && model.cost > 0 ? { costUsd: model.cost } : {}),
        date: day.date,
        // Fold cache into the input side so bar-counted total (input + output) includes cache.
        inputTokens: (model.inputTokens ?? 0) + (model.cacheReadTokens ?? 0) + (model.cacheCreationTokens ?? 0),
        model: model.modelName,
        outputTokens: model.outputTokens ?? 0,
        provider: 'claude-code' as const,
      })),
    )
  } else {
    console.log(`skipping claude on ${host} (no data or ccusage failed)`)
  }
  const codexRows: IngestRow[] = (codexReport.parse(JSON.parse(codexOut.stdout)).daily ?? []).flatMap((day) => {
    const models = Object.entries(day.models ?? {})
    const dayTokens = models.reduce((sum, [, usage]) => sum + (usage.totalTokens ?? 0), 0)
    return models.map(([model, usage]) => {
      // ccusage prices Codex per day, not per model, so apportion by token share.
      const costUsd = dayTokens > 0 ? (day.costUSD ?? 0) * ((usage.totalTokens ?? 0) / dayTokens) : 0
      return {
        ...(costUsd > 0 ? { costUsd } : {}),
        date: day.date,
        inputTokens: (usage.inputTokens ?? 0) + (usage.cacheReadTokens ?? 0) + (usage.cacheCreationTokens ?? 0),
        model,
        outputTokens: usage.outputTokens ?? 0,
        provider: 'codex' as const,
      }
    })
  })
  return [...claudeRows, ...codexRows]
}

// Sum per-account rows into one (provider, date, model) total. Correct because the
// accounts are disjoint, so no session is represented in more than one account's rows.
const mergeRows = (all: IngestRow[]): IngestRow[] => {
  const byKey = new Map<string, IngestRow & { hasCost: boolean }>()
  for (const row of all) {
    const key = `${row.provider}|${row.date}|${row.model}`
    const acc = byKey.get(key)
    if (!acc) {
      byKey.set(key, { ...row, hasCost: row.costUsd !== undefined })
      continue
    }
    acc.inputTokens += row.inputTokens
    acc.outputTokens += row.outputTokens
    if (row.costUsd !== undefined) {
      acc.costUsd = (acc.costUsd ?? 0) + row.costUsd
      acc.hasCost = true
    }
  }
  return [...byKey.values()].map(({ hasCost, ...row }) => (hasCost ? row : { ...row, costUsd: undefined }))
}

const run = async () => {
  const ingestUrl = process.env.USAGE_INGEST_URL ?? DEFAULT_INGEST_URL
  const computeHosts = process.env.USAGE_COMPUTE_HOSTS
  if (!computeHosts) {
    throw new Error('USAGE_COMPUTE_HOSTS is required: comma-separated ssh accounts holding the transcripts')
  }
  const hosts = [...new Set(computeHosts.split(',').filter(Boolean))]
  const optional = new Set((process.env.USAGE_OPTIONAL_HOSTS ?? '').split(',').filter(Boolean))
  const dryRun = process.env.USAGE_DRY_RUN === '1'

  // Additively push this Mac's sessions up to their topic account so its usage is in
  // the feed (no automatic Mac->server sync exists). Additive only -- no dedup/move of
  // the accounts' contents. Skipped for a compute-only dry run.
  if (!dryRun) {
    console.log('routing this Mac’s sessions up (additive) before compute...')
    await exec('python3', [`${import.meta.dir}/usage-repartition.py`, 'route'], { maxBuffer: MAX_BUFFER })
  }

  // Compute on every account concurrently. A required account that is unreachable (or
  // whose ccusage fails) ABORTS: silently dropping it would REPLACE-upsert lowered
  // totals. An optional account (its data already preserved elsewhere) is skipped
  // on failure instead.
  const perHost = await Promise.all(
    hosts.map(async (host) => {
      try {
        const rows = await rowsForHost(host)
        console.log(`computed ${host}: ${rows.length} (date, model) rows`)
        return rows
      } catch (error) {
        if (optional.has(host)) {
          console.log(`skipping optional ${host} (unreachable or ccusage failed)`)
          return [] as IngestRow[]
        }
        throw error
      }
    }),
  )

  const rows = mergeRows(perHost.flat())
  if (rows.length === 0) {
    console.log('no Codex (or residual Claude Code) usage found across the compute hosts')
    return
  }
  const tokens = rows.reduce((sum, row) => sum + row.inputTokens + row.outputTokens, 0)

  if (dryRun) {
    const claude = rows.filter((r) => r.provider === 'claude-code')
    const codex = rows.filter((r) => r.provider === 'codex')
    console.log(
      `DRY RUN: ${codex.length} codex + ${claude.length} claude-code merged (date, model) rows; ${tokens.toLocaleString()} tokens; not posted`,
    )
    return
  }

  const cronSecret = process.env.CRON_SECRET
  if (!cronSecret) {
    throw new Error('CRON_SECRET is required to authenticate the ingest push')
  }
  const response = await http.post(`${ingestUrl}${INGEST_PATH}`, {
    headers: { authorization: `Bearer ${cronSecret}` },
    json: { rows },
    timeout: 60_000,
  })
  if (!response.ok) {
    throw new Error(`ingest POST failed with ${response.status}: ${await response.text()}`)
  }
  const accepted = z
    .object({ accepted: z.number(), runId: z.string().min(1).optional() })
    .parse(await response.json())
  console.log(
    `pushed ${rows.length} merged (date, model) rows; ${tokens.toLocaleString()} tokens; server accepted ${accepted.accepted} rows (run ${accepted.runId ?? 'none'})`,
  )
  if (!accepted.runId) {
    return
  }

  // The upsert runs in a durable server-side workflow; poll so THIS process still
  // exits non-zero when the write ultimately fails (the `build` alias treats any throw
  // as a failed push). One transaction, settles in seconds; 60s is generous.
  for (let attempt = 0; attempt < 30; attempt += 1) {
    // eslint-disable-next-line no-await-in-loop -- sequential by design: a status poll
    await delay(2000)
    // eslint-disable-next-line no-await-in-loop -- sequential by design: a status poll
    const poll = await http.get(`${ingestUrl}${INGEST_PATH}?runId=${accepted.runId}`, {
      headers: { authorization: `Bearer ${cronSecret}` },
    })
    if (poll.status === 202) {
      continue
    }
    if (!poll.ok) {
      // eslint-disable-next-line no-await-in-loop -- sequential by design: a status poll
      throw new Error(`ingest workflow failed: ${poll.status}: ${await poll.text()}`)
    }
    // eslint-disable-next-line no-await-in-loop -- sequential by design: a status poll
    console.log(`ingest confirmed: ${JSON.stringify(await poll.json())}`)
    return
  }
  throw new Error('ingest workflow did not settle within 60s')
}

await run()
