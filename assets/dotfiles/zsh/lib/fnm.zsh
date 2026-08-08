# Pin is major `24`; pull a newer remote 24.x with `fnm install 24 && fnm
# default 24`. Idempotent once FNM_MULTISHELL_PATH is set.
if [[ -z "${FNM_MULTISHELL_PATH:-}" ]] && command -v fnm >/dev/null; then
  eval "$(fnm env --shell zsh)"
  # Retarget default to newest installed 24.x (no network).
  fnm default 24 >/dev/null 2>&1 || true
elif [[ -n "${FNM_MULTISHELL_PATH:-}" ]]; then
  # Already loaded upstream (.zshenv). Re-assert the front of PATH, because
  # lib/brew.zsh re-prepends ~/.local/bin afterwards and that shim's node is bun.
  path=("$FNM_MULTISHELL_PATH/bin" $path)
fi
