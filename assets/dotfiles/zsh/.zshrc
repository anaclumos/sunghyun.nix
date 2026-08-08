source "$ZSH_CONFIG_HOME/lib/brew.zsh"
source "$ZSH_CONFIG_HOME/lib/fnm.zsh"

export ZSH="$HOME/.oh-my-zsh"
ZSH_THEME="norm"
plugins=(
  git
  zsh-autosuggestions
  zsh-syntax-highlighting
)

source "$ZSH/oh-my-zsh.sh"

# norm's prompt shows only the hostname, which cannot tell two accounts on the
# same host apart.
PROMPT="${PROMPT/\%m/%n@%m}"

source "$ZSH_CONFIG_HOME/rc/aliases.zsh"
source "$ZSH_CONFIG_HOME/rc/functions.zsh"
source "$ZSH_CONFIG_HOME/rc/integrations.zsh"

[ -s "$HOME/.bun/_bun" ] && source "$HOME/.bun/_bun"

export BUN_INSTALL="$HOME/.bun"
export PATH="$BUN_INSTALL/bin:$PATH"
