#!/usr/bin/env bash
# Inside the container: Xvfb, then a D-Bus session for AT-SPI, Orca and Chromium.
set -u
export XDG_RUNTIME_DIR=/tmp/xdg HOME=/tmp/home
mkdir -p "$XDG_RUNTIME_DIR" "$HOME" && chmod 700 "$XDG_RUNTIME_DIR"
# The container shares the host's network, and so its abstract X sockets:
# let Xvfb pick a free display and report it once it accepts connections.
Xvfb -displayfd 3 -screen 0 1440x900x24 -nolisten tcp 3>/tmp/xvfb-display >/out/xvfb.log 2>&1 &
for _ in $(seq 300); do
  [[ -s /tmp/xvfb-display ]] && break
  sleep 0.1
done
DISPLAY=":$(head -n 1 /tmp/xvfb-display)"
[[ "$DISPLAY" =~ ^:[0-9]+$ ]] || { echo "Xvfb reported no display" >&2; exit 1; }
export DISPLAY
exec dbus-run-session -- bash /work/inner.sh
