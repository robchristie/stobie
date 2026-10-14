#!/usr/bin/env bash
# Inside the container: Xvfb, then a D-Bus session for AT-SPI, Orca and Chromium.
set -u
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/xdg HOME=/tmp/home
mkdir -p "$XDG_RUNTIME_DIR" "$HOME" && chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 1440x900x24 -nolisten tcp >/out/xvfb.log 2>&1 &
sleep 1
exec dbus-run-session -- bash /work/inner.sh
