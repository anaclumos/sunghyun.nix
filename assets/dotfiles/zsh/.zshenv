export ZSH_CONFIG_HOME="$HOME/.config/zsh"

# Ubuntu's /etc/zsh/zshrc runs a raw global compinit, which prompts about
# "insecure" dirs on login shells for accounts that do not own the Homebrew
# prefix. oh-my-zsh runs compinit itself.
skip_global_compinit=1
export HOMEBREW_NO_ENV_HINTS=1
export HOMEBREW_NO_ASK=1
# Block global pip installs; pair with python/python3 wrappers below so bare
# interpreters also fail outside an activated venv. Python work goes through uv.
export PIP_REQUIRE_VIRTUALENV=true
export OLLAMA_ORIGINS="*"

# Shell-invoked python/python3 only (shebangs and execve still find the binary).
python() {
  if [[ -z "${VIRTUAL_ENV:-}" ]]; then
    print -u2 "error: python requires an activated venv; use: uv run ...  or  uv venv && source .venv/bin/activate"
    return 1
  fi
  command python "$@"
}
python3() {
  if [[ -z "${VIRTUAL_ENV:-}" ]]; then
    print -u2 "error: python3 requires an activated venv; use: uv run ...  or  uv venv && source .venv/bin/activate"
    return 1
  fi
  command python3 "$@"
}

if [[ -r "$HOME/.ssh/.secrets.env" ]]; then
  . "$HOME/.ssh/.secrets.env"
fi

if [[ -r "$HOME/.cargo/env" ]]; then
  . "$HOME/.cargo/env"
fi

typeset -gU path PATH
path=(
  "$HOME/.local/bin"
  $path
)
export PATH

# After the ~/.local/bin prepend, so fnm's node outranks the bun-backed
# ~/.local/bin/node shim. Non-login, non-interactive shells get their Node here;
# login and interactive shells re-source this and it is a no-op past the first.
source "$ZSH_CONFIG_HOME/lib/fnm.zsh"
