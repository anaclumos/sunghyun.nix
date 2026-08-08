# Idempotent: re-running is a no-op once HOMEBREW_PREFIX is set.
if [[ -z "${HOMEBREW_PREFIX:-}" ]]; then
  for _brew in /opt/homebrew/bin/brew /usr/local/bin/brew /home/linuxbrew/.linuxbrew/bin/brew; do
    if [[ -x "$_brew" ]]; then
      eval "$("$_brew" shellenv zsh)"
      # Keep ~/.local/bin ahead of the Homebrew paths shellenv just prepended.
      path=("$HOME/.local/bin" $path)
      break
    fi
  done
  unset _brew
fi
