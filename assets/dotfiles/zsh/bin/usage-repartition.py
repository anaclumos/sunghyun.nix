#!/usr/bin/env python3
"""Keep the consolidation server's accounts a complete, disjoint feed for the usage pusher.

The server, its accounts, and the topic routing are machine-local environment
values (deliberately never committed):

  USAGE_COMPUTE_HOSTS  required: comma-separated ssh accounts holding transcripts
  USAGE_OPTIONAL_HOSTS optional: hosts skipped (not fatal) when unreachable,
                       because their data is already preserved elsewhere
  USAGE_ELSE_HOST      required: ssh account receiving sessions with no topic match
  USAGE_TOPIC_ROUTES   required: comma-separated topic=account pairs; a session
                       whose project path contains the topic substring routes to
                       that account (first match wins, in the listed order)

Two independent jobs:

  route  (default; the pusher runs this)
      Push THIS Mac's transcripts up to the server so its usage is counted. There
      is no automatic Mac->server sync, so without this the Mac's new sessions are
      dropped. Each session is sent to its TOPIC account. This is purely ADDITIVE
      (rsync, no --delete): re-sending an existing session is a no-op union, and
      because every session always goes to the same one account it never creates a
      cross-account duplicate. It does NOT touch the accounts' existing contents.

  heal   (manual; run only if drift ever reintroduces a cross-account duplicate)
      Classify every account, bridge each misfiled session into its target account, then
      reversibly remove the misfiled copies -- but only after the target is confirmed to
      hold it (destination-verified, never an unbacked delete). Removed copies go to
      ~/.repartition-holding on each account. No cross-account read: the Mac bridges.

An optional host that is unreachable is skipped.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
from collections import defaultdict


def env_required(name):
    value = os.environ.get(name)
    if not value:
        sys.exit(f"{name} is required; set it in the machine-local environment")
    return value


def _local_names():
    host = socket.gethostname()
    short = host.split(".", 1)[0]
    names = {host, short, "localhost", "local"}
    try:
        out = subprocess.run(
            ["scutil", "--get", "LocalHostName"],
            capture_output=True,
            text=True,
            check=False,
        )
        if out.returncode == 0 and out.stdout.strip():
            names.add(out.stdout.strip())
    except OSError:
        pass
    return names


LOCAL_NAMES = _local_names()


def host_is_local(host):
    name = host.split("@", 1)[-1]
    return name in LOCAL_NAMES


HOSTS = env_required("USAGE_COMPUTE_HOSTS").split(",")
OPTIONAL = set(filter(None, os.environ.get("USAGE_OPTIONAL_HOSTS", "").split(",")))
ELSE_ACCT = env_required("USAGE_ELSE_HOST").split("@", 1)[0]
STATE = os.path.join(tempfile.gettempdir(), "usage-repartition")
_nix_rsync = shutil.which("rsync")
RSYNC = (
    "/opt/homebrew/bin/rsync"
    if os.path.exists("/opt/homebrew/bin/rsync")
    else (_nix_rsync or "rsync")
)
CM = "-o ControlMaster=auto -o ControlPath=/tmp/urp-cm-%C -o ControlPersist=180"
SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10"] + CM.split()
SSH_E = "ssh -o BatchMode=yes -o ConnectTimeout=10 " + CM
RS = [RSYNC, "-rltz", "--timeout=120", "-e", SSH_E]
CHURN_OK = (0, 23, 24)  # a source that vanished mid-run (live account) is tolerated

ACCT_HOST = {h.split("@", 1)[0]: h for h in HOSTS if h}
TARGET = dict(pair.split("=", 1) for pair in env_required("USAGE_TOPIC_ROUTES").split(",") if pair)

CLASSIFY_SRC = r'''
import json, os, sys
HOME = os.path.expanduser("~")
TOPICS = __TOPICS__
def target(cwd):
    for t in TOPICS:
        if t in cwd: return t
    return "else"
cdir = os.path.join(HOME, ".claude", "projects")
if os.path.isdir(cdir):
    for proj in os.listdir(cdir):
        pp = os.path.join(cdir, proj)
        if not os.path.isdir(pp): continue
        t = target(proj)
        for f in os.listdir(pp):
            if f.endswith(".jsonl") and os.path.isfile(os.path.join(pp, f)):
                sys.stdout.write("claude\t%s\t%s\t%s\n" % (proj, f, t))
xdir = os.path.join(HOME, ".codex", "sessions")
if os.path.isdir(xdir):
    for root, _d, files in os.walk(xdir):
        for f in files:
            if not f.endswith(".jsonl"): continue
            cwd = ""
            try:
                with open(os.path.join(root, f)) as fh: cwd = (json.loads(fh.readline()).get("payload") or {}).get("cwd") or ""
            except Exception: cwd = ""
            sys.stdout.write("codex\t%s\t%s\t%s\n" % (os.path.relpath(root, xdir), f, target(cwd)))
'''.replace("__TOPICS__", json.dumps(list(TARGET)))


def p(*a):
    print(*a, flush=True)


def run(cmd, ok=(0,)):
    rc = subprocess.run(cmd).returncode
    if rc not in ok:
        sys.exit(f"FAIL {rc}: {' '.join(cmd[:5])}...")
    return rc


def rsh(host, script, payload=""):
    if payload and not payload.endswith("\n"):
        payload += "\n"
    if subprocess.run(SSH + [host, script], input=payload, text=True).returncode != 0:
        sys.exit(f"remote FAIL on {host}")


def reachable(host):
    if host_is_local(host):
        return True
    return subprocess.run(SSH + [host, "true"]).returncode == 0


def target_acct(topic, accts):
    a = TARGET.get(topic, ELSE_ACCT)
    return a if a in accts else (ELSE_ACCT if ELSE_ACCT in accts else None)


def classify(reader, out):
    with open(out, "w") as fh:
        if subprocess.run(reader, input=CLASSIFY_SRC, stdout=fh, text=True).returncode != 0:
            sys.exit(f"classify FAIL: {reader}")
    return out


def reachable_accts():
    accts = []
    for a, h in ACCT_HOST.items():
        if reachable(h):
            accts.append(a)
        elif h in OPTIONAL:
            p(f"[skip] optional {h} unreachable")
        else:
            sys.exit(f"required host {h} unreachable")
    return accts


def route(accts):
    cls = classify(["python3", "-"], f"{STATE}/cls-MAC.txt")
    claude_by_dst = defaultdict(list)   # dst acct -> [projdir]
    codex_by_dst = defaultdict(list)    # dst acct -> [reldir/base]
    for ln in open(cls):
        tool, path, base, topic = ln.rstrip("\n").split("\t")
        dst = target_acct(topic, accts)
        if dst is None:
            continue  # target account unreachable this run; leave on Mac (counted next time)
        if tool == "claude":
            if path not in claude_by_dst[dst]:
                claude_by_dst[dst].append(path)
        else:
            codex_by_dst[dst].append(f"{path}/{base}")
    for dst, projs in claude_by_dst.items():
        dest = ACCT_HOST[dst]
        if host_is_local(dest):
            p(f"[route] claude -> {dst}: {len(projs)} project dirs (already local; skip)")
            continue
        p(f"[route] claude -> {dst}: {len(projs)} project dirs")
        for pj in projs:
            run(RS + [os.path.expanduser(f"~/.claude/projects/{pj}") + "/", f"{dest}:.claude/projects/{pj}/"], ok=CHURN_OK)
    for dst, files in codex_by_dst.items():
        dest = ACCT_HOST[dst]
        if host_is_local(dest):
            p(f"[route] codex -> {dst}: {len(files)} sessions (already local; skip)")
            continue
        lst = f"{STATE}/route-codex-{dst}.lst"
        open(lst, "w").write("\n".join(files) + "\n")
        p(f"[route] codex -> {dst}: {len(files)} sessions")
        run(RS + ["--files-from", lst, os.path.expanduser("~/.codex/sessions") + "/", f"{dest}:.codex/sessions/"], ok=CHURN_OK)
    p("[route] DONE")


def heal(accts):
    def load():
        claude, codex = {}, {}
        for a in accts:
            for ln in open(classify(SSH + [ACCT_HOST[a], "python3 -"], f"{STATE}/cls-{a}.txt")):
                tool, path, base, topic = ln.rstrip("\n").split("\t")
                d = (claude.setdefault(path, {"t": None, "h": set()}) if tool == "claude"
                     else codex.setdefault((path, base), {"t": None, "h": set()}))
                d["t"] = target_acct(topic, accts)
                d["h"].add(a)
        return claude, codex

    claude, codex = load()
    jobs = [(pj, a, d["t"]) for pj, d in claude.items() for a in d["h"] if d["t"] and a != d["t"]]
    p(f"[heal] claude bridges: {len(jobs)}")
    for pj, src, dst in jobs:
        sp = f"{STATE}/stage/claude/{pj}"
        os.makedirs(sp, exist_ok=True)
        if run(RS + [f"{ACCT_HOST[src]}:.claude/projects/{pj}/", f"{sp}/"], ok=CHURN_OK) != 0:
            continue
        run(RS + [f"{sp}/", f"{ACCT_HOST[dst]}:.claude/projects/{pj}/"], ok=CHURN_OK)
    groups = defaultdict(list)
    for (rel, base), d in codex.items():
        if d["t"] and d["t"] not in d["h"]:
            groups[(sorted(d["h"])[0], d["t"])].append(f"{rel}/{base}")
    os.makedirs(f"{STATE}/stage/codex", exist_ok=True)
    for (src, dst), files in groups.items():
        lst = f"{STATE}/heal-codex-{src}-{dst}.lst"
        open(lst, "w").write("\n".join(files) + "\n")
        run(RS + ["--files-from", lst, f"{ACCT_HOST[src]}:.codex/sessions/", f"{STATE}/stage/codex/"], ok=CHURN_OK)
        run(RS + ["--files-from", lst, f"{STATE}/stage/codex/", f"{ACCT_HOST[dst]}:.codex/sessions/"], ok=CHURN_OK)
    # re-classify so removal is destination-verified against post-move holders
    claude, codex = load()
    CL = ('set -eu\ncd "$HOME/.claude/projects"\nwhile IFS= read -r P; do [ -e "$P" ] || continue; '
          'd="$HOME/.repartition-holding/claude"; mkdir -p "$d"; '
          'if [ -e "$d/$P" ]; then mv -- "$P" "$d/${P}.dup.$(date +%s%N)"; else mv -- "$P" "$d/"; fi; done\n')
    CX = ('set -eu\ncd "$HOME/.codex/sessions"\nwhile IFS= read -r f; do [ -e "$f" ] || continue; '
          'd="$HOME/.repartition-holding/codex/${f%/*}"; mkdir -p "$d"; b="${f##*/}"; '
          'if [ -e "$d/$b" ]; then mv -- "$f" "$d/${b}.dup.$(date +%s%N)"; else mv -- "$f" "$d/"; fi; done\n')
    for a in accts:
        pjs = [pj for pj, d in claude.items() if a in d["h"] and d["t"] and d["t"] != a and d["t"] in d["h"]]
        cxs = [f"{r}/{b}" for (r, b), d in codex.items() if a in d["h"] and d["t"] and d["t"] != a and d["t"] in d["h"]]
        p(f"[heal] {a}: park {len(pjs)} claude dirs, {len(cxs)} codex files")
        if pjs:
            rsh(ACCT_HOST[a], CL, "\n".join(pjs))
        for i in range(0, len(cxs), 4000):
            rsh(ACCT_HOST[a], CX, "\n".join(cxs[i:i + 4000]))
    p("[heal] DONE")


def main():
    os.makedirs(STATE, exist_ok=True)
    phase = sys.argv[1] if len(sys.argv) > 1 else "route"
    accts = reachable_accts()
    if phase in ("route", "heal"):
        route(accts)
    if phase == "heal":
        heal(accts)


if __name__ == "__main__":
    main()
