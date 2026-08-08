die() {
  echo >&2 "$*"
  exit 1
}

skip() {
  echo >&2 "skipped: $*"
  exit 0
}

usage() {
  cat <<'USAGE'
sunghyun — keyboard actions plus the residual OS-prompt surfaces (Nix owns the rest)

  open <target>                 open an app by key, bundle id, or name
  open-default-browser          activate the OS default web browser (Hyper+J)
  toggle-dark-mode              flip the system appearance (Hyper+`)
  default-browser [status|set]  report, or ask macOS to change, the default browser
  input-source <name>           switch input source (ABC / 2SetKorean / raw TIS id)
  tile <action>                 place the focused window
  spotlight <subcommand>        ⌘Space, Clipboard History, terminal→Ghostty alias
  fn-state [status|apply]       push the declared top-row fn behaviour into IOHIDSystem
  hotkeys [status|apply]        free the chords reserved for apps (⌘⇧Space → 1Password)
  karabiner health              detect and heal a Core-Service that missed a config relink
  kanata [status|disable|enable --safe]
                                opt-in keyboard engine, proof-gated
  virt                          report virtualization; exit 0 in a VM, 1 on bare metal
  verify [--json]               assert the declared outcomes hold live
  post-switch [--dry-run] [--json]
                                residual steps after darwin-rebuild switch
USAGE
}

main() {
  [ $# -gt 0 ] || {
    usage
    exit 1
  }
  local command="$1"
  shift
  case "$command" in
    open) cmd_open "$@" ;;
    open-default-browser | open-browser) cmd_open_default_browser ;;
    toggle-dark-mode) cmd_toggle_dark_mode ;;
    default-browser) cmd_default_browser "$@" ;;
    input-source) cmd_input_source "$@" ;;
    tile) cmd_tile "$@" ;;
    spotlight) cmd_spotlight "$@" ;;
    fn-state) cmd_fn_state "$@" ;;
    hotkeys) cmd_hotkeys "$@" ;;
    karabiner) cmd_karabiner "$@" ;;
    kanata) cmd_kanata "$@" ;;
    virt) cmd_virt ;;
    verify) cmd_verify "$@" ;;
    post-switch) cmd_post_switch "$@" ;;
    -h | --help | help) usage ;;
    --version | version) echo "sunghyun @version@" ;;
    *)
      usage
      die "unknown command: $command"
      ;;
  esac
}
