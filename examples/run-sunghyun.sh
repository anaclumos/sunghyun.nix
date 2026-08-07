#!/bin/bash
# Kanata LaunchDaemon runs as root (VirtualHID rootonly IPC). Root has no Aqua
# session, so sunghyun would detect headless and skip IME/tile/open.
# Re-run as the console user inside their GUI bootstrap namespace.
set -euo pipefail
bin=/usr/local/bin/sunghyun
if [[ ! -x $bin ]]; then
  bin=$(command -v sunghyun)
fi
uid=$(stat -f %u /dev/console)
if [[ $(id -u) -eq 0 && "$uid" != "0" ]]; then
  exec /bin/launchctl asuser "$uid" "$bin" "$@"
fi
exec "$bin" "$@"
