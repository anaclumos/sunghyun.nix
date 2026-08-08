hn() {
  emulate -L zsh
  local limit=${1:-30}
  local cutoff=$(($(date +%s) - 86400))
  local ids item_json time_val url title opened=0
  local opener=${commands[open]:-${commands[xdg-open]:-}}

  if [[ -z "$opener" ]]; then
    print -u2 "hn: no URL opener found (open or xdg-open)"
    return 1
  fi

  ids=$(curl -s "https://hacker-news.firebaseio.com/v0/topstories.json") || {
    print -u2 "hn: failed to fetch top stories"
    return 1
  }

  for id in $(print "$ids" | tr -d '[]' | tr ',' '\n' | head -n 200); do
    item_json=$(curl -s "https://hacker-news.firebaseio.com/v0/item/${id}.json") || continue
    time_val=$(print "$item_json" | sed -n 's/.*"time":\([0-9]*\).*/\1/p')
    [[ -z "$time_val" || "$time_val" -lt "$cutoff" ]] && continue

    url=$(print "$item_json" | sed -n 's/.*"url":"\([^"]*\)".*/\1/p')
    [[ -z "$url" ]] && url="https://news.ycombinator.com/item?id=${id}"

    title=$(print "$item_json" | sed -n 's/.*"title":"\([^"]*\)".*/\1/p')
    print "${title:-untitled}: $url"
    "$opener" "$url"
    opened=$((opened + 1))
    [[ "$opened" -ge "$limit" ]] && break
  done

  print "hn: opened $opened stories"
}

_build_run_step() {
  emulate -L zsh

  local dry_run=$1
  local label=$2
  shift 2

  print "=== $label ==="

  if (( dry_run )); then
    local arg
    printf 'Dry running:'
    for arg in "$@"; do
      printf ' %q' "$arg"
    done
    printf '\n'
    return 0
  fi

  "$@"
}

_build_pull_dev_repos() {
  emulate -L zsh

  local dry_run=$1
  local dev_dir="${HOME}/Developer"
  local repo
  local exit_code=0

  [[ ! -d "$dev_dir" ]] && return 0

  for repo in "$dev_dir"/*(N/); do
    [[ ! -e "${repo}/.git" ]] && continue
    git -C "$repo" rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1 || continue
    _build_run_step "$dry_run" "git pull: ${repo:t}" git -C "$repo" pull --ff-only || exit_code=1
  done

  return $exit_code
}

_build_sunghyun_dir() {
  emulate -L zsh
  local dir="${SUNGHYUN_DIR:-${HOME}/Developer/sunghyun.nix}"
  if [[ ! -d "${dir}/.git" || ! -f "${dir}/flake.nix" ]]; then
    print -u2 "build: sunghyun.nix checkout missing at ${dir} (set SUNGHYUN_DIR)"
    return 1
  fi
  print -r -- "$dir"
}

_build_flake_host() {
  emulate -L zsh
  local repo_dir=$1
  local host="${SUNGHYUN_HOST:-}"
  local local_host

  if [[ -n "$host" ]]; then
    print -r -- "$host"
    return 0
  fi
  local_host="$(scutil --get LocalHostName 2>/dev/null || true)"
  if [[ -n "$local_host" && -f "${repo_dir}/nix/darwin/hosts/${local_host}.nix" ]]; then
    print -r -- "$local_host"
    return 0
  fi
  print -r -- "default"
}

_build_linux_home_attr() {
  emulate -L zsh
  case "$(uname -m)" in
    aarch64|arm64) print -r -- 'sc@aarch64-linux' ;;
    x86_64|amd64) print -r -- 'sc@x86_64-linux' ;;
    *)
      print -u2 "build: unsupported Linux architecture: $(uname -m)"
      return 1
      ;;
  esac
}

# Pathspec-only flake.lock commit so a dirty shared checkout never sweeps
# concurrent WIP. Leave the lock dirty (and print) if commit or push fails.
_build_commit_flake_lock() {
  emulate -L zsh
  local repo_dir=$1
  local dry_run=$2
  local branch status_line

  if (( dry_run )); then
    _build_run_step 1 "git commit+push flake.lock" \
      git -C "$repo_dir" commit flake.lock -m "chore(flake): refresh flake.lock"
    return 0
  fi

  git -C "$repo_dir" diff --quiet -- flake.lock \
    && git -C "$repo_dir" diff --cached --quiet -- flake.lock && {
    print "build: flake.lock unchanged; nothing to commit"
    return 0
  }

  branch="$(git -C "$repo_dir" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
  if [[ "$branch" != "main" ]]; then
    print -u2 "build: on ${branch:-unknown}, not main; leaving flake.lock dirty for a manual commit"
    return 0
  fi

  status_line="$(git -C "$repo_dir" status -sb 2>/dev/null || true)"
  if [[ "$status_line" != *'main...origin/main'* && "$status_line" != '## main' ]]; then
    print -u2 "build: unexpected branch tracking (${status_line}); leaving flake.lock dirty"
    return 0
  fi

  print "=== git commit+push flake.lock ==="
  git -C "$repo_dir" add -- flake.lock || {
    print -u2 "build: failed to stage flake.lock; leaving it dirty"
    return 1
  }
  if git -C "$repo_dir" diff --cached --quiet -- flake.lock; then
    print "build: flake.lock unchanged after stage; nothing to commit"
    return 0
  fi
  git -C "$repo_dir" commit -m "$(cat <<'EOF'
chore(flake): refresh flake.lock

nix flake update from the build shell function.
EOF
)" -- flake.lock || {
    print -u2 "build: commit failed; flake.lock left staged/dirty for a manual commit"
    return 1
  }
  git -C "$repo_dir" push origin main || {
    print -u2 "build: push failed after committing flake.lock; pull/rebase and push manually"
    return 1
  }
}

_build_nix_switch() {
  emulate -L zsh
  local dry_run=$1
  local repo_dir=$2
  local host hm out

  case "$(uname -s)" in
    Darwin)
      host="$(_build_flake_host "$repo_dir")" || return 1
      if ! command -v darwin-rebuild >/dev/null 2>&1; then
        print -u2 "build: darwin-rebuild not on PATH; run install.sh once or open a login shell after a prior switch"
        return 1
      fi
      _build_run_step "$dry_run" "darwin-rebuild switch --flake .#${host}" \
        sudo darwin-rebuild switch --flake "${repo_dir}#${host}" || return 1
      if (( ! dry_run )) && command -v sunghyun >/dev/null 2>&1; then
        _build_run_step 0 "sunghyun karabiner health" sunghyun karabiner health || return 1
      elif (( dry_run )); then
        _build_run_step 1 "sunghyun karabiner health" sunghyun karabiner health
      fi
      ;;
    Linux)
      hm="$(_build_linux_home_attr)" || return 1
      if (( dry_run )); then
        _build_run_step 1 "home-manager activate .#${hm}" \
          nix build --no-link --print-out-paths \
          "${repo_dir}#homeConfigurations.\"${hm}\".activationPackage"
        return 0
      fi
      print "=== home-manager activate .#${hm} ==="
      out="$(nix build --no-link --print-out-paths \
        "${repo_dir}#homeConfigurations.\"${hm}\".activationPackage")" || return 1
      HOME_MANAGER_BACKUP_EXT=hm-backup "${out}/activate" || return 1
      ;;
    *)
      print -u2 "build: unsupported OS: $(uname -s)"
      return 1
      ;;
  esac
}

build() {
  emulate -L zsh
  setopt pipefail

  local arg
  local dry_run=0
  local exit_code=0
  local repo_dir

  for arg in "$@"; do
    case "$arg" in
      -h|--help)
        cat <<'EOF'
Usage: build [options]

Upgrade this machine the Nix way: refresh flake inputs, then build and switch
the live host (darwin-rebuild on macOS, home-manager on Linux). Declared
Homebrew packages upgrade during activation via homebrew.onActivation.upgrade.
topgrade is not part of this path.

Options:
  -n, --dry-run  Print what would run.
  -h, --help     Show this help.

Repo: $SUNGHYUN_DIR (default ~/Developer/sunghyun.nix)
Host: $SUNGHYUN_HOST, else LocalHostName when nix/darwin/hosts/<name>.nix
      exists, else default. Linux home attr follows uname -m.
EOF
        return 0
        ;;
      -n|--dry-run)
        dry_run=1
        ;;
      *)
        print -u2 "build: unknown option: $arg (see --help)"
        return 1
        ;;
    esac
  done

  repo_dir="$(_build_sunghyun_dir)" || return 1

  _build_pull_dev_repos "$dry_run" || exit_code=1

  # Exactly one designated Mac may push: the ingest REPLACE-upserts per-day
  # totals, so a second pusher would clobber them. Hence the env gate.
  local cho_usage_push="${ZSH_CONFIG_HOME}/bin/usage-push.ts"
  if [[ -n "${USAGE_COMPUTE_HOSTS:-}" ]] && [[ -f "$cho_usage_push" ]] && command -v bun >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    _build_run_step "$dry_run" "cho.sh usage push" zsh -c "cd ${cho_usage_push:h} && bun run ${cho_usage_push:t}" || exit_code=1
  fi

  # Separate writer from the push above, gated on its own key.
  local cho_app="${CHO_SH_APP:-${HOME}/Developer/cho.sh/app}"
  local cho_cursor_push="${cho_app}/scripts/cursor-usage-push.ts"
  local cho_cursor_export="${ZSH_CONFIG_HOME}/bin/cursor-usage-export.py"
  if [[ -n "${CURSOR_MANAGEMENT_KEY:-}" ]] && [[ -f "$cho_cursor_push" ]] && [[ -f "$cho_cursor_export" ]] && command -v bun >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    _build_run_step "$dry_run" "cho.sh cursor usage push" \
      env CURSOR_USAGE_EXPORT_PY="$cho_cursor_export" \
      zsh -c "cd ${(q)cho_app} && bun run scripts/cursor-usage-push.ts" || exit_code=1
  fi

  (
    cd "$repo_dir" || exit 1
    _build_run_step "$dry_run" "nix flake update" nix flake update || exit 1
  ) || exit_code=1

  if (( exit_code == 0 )); then
    _build_nix_switch "$dry_run" "$repo_dir" || exit_code=1
  fi

  if (( exit_code == 0 )); then
    _build_commit_flake_lock "$repo_dir" "$dry_run" || exit_code=1
  else
    print -u2 "build: skipping flake.lock commit because an earlier step failed"
  fi

  return $exit_code
}

emptyfolder() {
  find . -type d -empty -delete
}