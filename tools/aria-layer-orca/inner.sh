#!/usr/bin/env bash
# Start AT-SPI, Orca with a debug transcript and Chromium on the served
# application, then drive a short keyboard journey with xdotool.
set -u
export GTK_MODULES=gail:atk-bridge NO_AT_BRIDGE=0 ACCESSIBILITY_ENABLED=1
/usr/lib/at-spi-bus-launcher --launch-immediately >/out/atspi.log 2>&1 &
sleep 1
gsettings set org.gnome.desktop.interface toolkit-accessibility true
gsettings set org.gnome.desktop.a11y.applications screen-reader-enabled true
# Without an audio device a say-all never completes, and Orca then ignores
# focus changes; the journey reads by focus instead.
for key in say-all-on-load page-summary-on-load; do
  gsettings set org.gnome.Orca.Document:/org/gnome/orca/default/document/ "$key" false
done
{
  orca --version
  pacman -Q orca at-spi2-core speech-dispatcher espeak-ng xorg-server-xvfb
  /chromium/chrome --version
} >/out/versions.txt 2>&1
(cd /out && orca --replace --debug --debug-file=/out/orca-debug.txt >/out/orca.log 2>&1) &
sleep 6
/chromium/chrome --no-sandbox --force-renderer-accessibility --ozone-platform=x11 \
  --user-data-dir=/tmp/profile --no-first-run --window-position=0,0 --window-size=1440,900 \
  --enable-unsafe-webgpu --use-webgpu-adapter=swiftshader --enable-dawn-features=allow_unsafe_apis \
  --use-vulkan=swiftshader --enable-unsafe-swiftshader --enable-features=Vulkan \
  --disable-vulkan-fallback-to-gl-for-testing --use-gpu-in-tests \
  "$APP_URL" >/out/chrome.log 2>&1 &
sleep "${STARTUP_WAIT:-50}"
step() { echo "$(date +%T.%N) $*" >>/out/steps.log; }
key() { step "$*"; xdotool key --delay 150 "$@"; sleep 3; }
window=$(xdotool search --onlyvisible --class chromium | head -1)
xdotool windowactivate --sync "$window" 2>/dev/null; xdotool windowfocus "$window"
step "click the canvas"; xdotool mousemove 1200 860 click 1; sleep 3
key ctrl
key Tab
key Tab
key Tab
key ctrl+s
key ctrl+f
key Escape
sleep 3
pkill -f orca; pkill -f chrome
