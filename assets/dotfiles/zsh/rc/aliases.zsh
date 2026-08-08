alias s="source ~/.zshrc"

alias rm="rm -i"
unsetopt RM_STAR_SILENT
setopt RM_STAR_WAIT
alias x="exit"
alias c="cursor"
alias "c."="cursor ."
alias z='cursor "$HOME/Developer/sunghyun.nix"'
cc() {
  claude --dangerously-skip-permissions "$@"
}
alias uncommit="git reset --soft HEAD~1"
alias sentry-cli="echo 'Use \`sentry --help\`'"
