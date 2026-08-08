#!/bin/bash
# The Kanata daemon runs as root, which has no Aqua session, so sunghyun would
# detect headless and skip IME/tile/open. Re-enter the console user's GUI
# bootstrap namespace first.
set -euo pipefail
bin=/run/current-system/sw/bin/sunghyun
if [[ ! -x $bin ]]; then
  bin=$(command -v sunghyun)
fi
uid=$(stat -f %u /dev/console)
if [[ $(id -u) -eq 0 && "$uid" != "0" ]]; then
  exec /bin/launchctl asuser "$uid" "$bin" "$@"
fi
exec "$bin" "$@"
