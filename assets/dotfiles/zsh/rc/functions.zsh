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

_build_has_container_runtime() {
  emulate -L zsh

  if command -v docker >/dev/null 2>&1 && command docker info >/dev/null 2>&1; then
    return 0
  fi

  if command -v podman >/dev/null 2>&1 && command podman info >/dev/null 2>&1; then
    return 0
  fi

  return 1
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

build() {
  emulate -L zsh
  setopt pipefail

  local arg extension
  local dry_run=0
  local greedy=0
  local no_system=0
  local force_containers=0
  local brew_managed_az=0
  local only_steps=0
  local exit_code=0
  local step_selector=""
  local -a az_extensions topgrade_args

  topgrade_args=(
    --cleanup
    --show-skipped
    --no-ask-retry
    --disable pip3
    --disable pip_review
    --disable pip_review_local
    --disable pipupgrade
  )

  for arg in "$@"; do
    if [[ "$arg" == -* && "$arg" != --only && "$arg" != --disable ]]; then
      step_selector=""
    fi

    case "$arg" in
      -h|--help)
        cat <<'EOF'
Usage: build [options] [topgrade args...]

Options:
  --greedy     Force Homebrew to upgrade self-updating and :latest casks too.
  --no-system  Skip macOS software updates.
  -n, --dry-run
               Print what would run.
  -h, --help   Show this help.

Everything else is forwarded to topgrade.
EOF
        return 0
        ;;
      --greedy)
        greedy=1
        ;;
      --no-system)
        no_system=1
        ;;
      -n|--dry-run)
        dry_run=1
        topgrade_args+=("$arg")
        ;;
      --only|--disable)
        step_selector=${arg#--}
        force_containers=1
        topgrade_args+=("$arg")
        ;;
      containers)
        [[ "$step_selector" == only ]] && only_steps=1
        force_containers=1
        topgrade_args+=("$arg")
        ;;
      *)
        [[ "$step_selector" == only ]] && only_steps=1
        topgrade_args+=("$arg")
        ;;
    esac
  done

  if ! command -v topgrade >/dev/null 2>&1; then
    print -u2 "build: topgrade not found. Install it with 'brew install topgrade'."
    return 1
  fi

  if (( no_system )); then
    topgrade_args+=(--disable system)
  fi

  if (( ! force_containers )) && ! _build_has_container_runtime; then
    topgrade_args+=(--disable containers)
  fi

  if command -v brew >/dev/null 2>&1 && command brew list --versions azure-cli >/dev/null 2>&1; then
    brew_managed_az=1
  fi

  if (( ! only_steps )); then
    _build_pull_dev_repos "$dry_run" || exit_code=1
  fi

  # Exactly one designated Mac may push: the ingest REPLACE-upserts per-day
  # totals, so a second pusher would clobber them. Hence the env gate.
  local cho_usage_push="${ZSH_CONFIG_HOME}/bin/usage-push.ts"
  if (( ! only_steps )) && [[ -n "${USAGE_COMPUTE_HOSTS:-}" ]] && [[ -f "$cho_usage_push" ]] && command -v bun >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    _build_run_step "$dry_run" "cho.sh usage push" zsh -c "cd ${cho_usage_push:h} && bun run ${cho_usage_push:t}" || exit_code=1
  fi

  # Separate writer from the push above, gated on its own key.
  local cho_app="${CHO_SH_APP:-${HOME}/Developer/cho.sh/app}"
  local cho_cursor_push="${cho_app}/scripts/cursor-usage-push.ts"
  local cho_cursor_export="${ZSH_CONFIG_HOME}/bin/cursor-usage-export.py"
  if (( ! only_steps )) && [[ -n "${CURSOR_MANAGEMENT_KEY:-}" ]] && [[ -f "$cho_cursor_push" ]] && [[ -f "$cho_cursor_export" ]] && command -v bun >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    _build_run_step "$dry_run" "cho.sh cursor usage push" \
      env CURSOR_USAGE_EXPORT_PY="$cho_cursor_export" \
      zsh -c "cd ${(q)cho_app} && bun run scripts/cursor-usage-push.ts" || exit_code=1
  fi

  if (( greedy )); then
    _build_run_step "$dry_run" "topgrade" env HOMEBREW_UPGRADE_GREEDY=1 topgrade "${topgrade_args[@]}" || exit_code=1
  else
    _build_run_step "$dry_run" "topgrade" topgrade "${topgrade_args[@]}" || exit_code=1
  fi

  if (( ! only_steps )) && command -v az >/dev/null 2>&1; then
    if (( brew_managed_az )); then
      az_extensions=("${(@f)$(az extension list --query '[].name' -o tsv 2>/dev/null)}")
      for extension in "${az_extensions[@]}"; do
        [[ -z "$extension" ]] && continue
        _build_run_step "$dry_run" "Azure CLI extension: $extension" az extension update --name "$extension" --only-show-errors || exit_code=1
      done
    else
      _build_run_step "$dry_run" "Azure CLI" az upgrade --yes --all --only-show-errors || exit_code=1
    fi
  fi

  return $exit_code
}

emptyfolder() {
  find . -type d -empty -delete
}