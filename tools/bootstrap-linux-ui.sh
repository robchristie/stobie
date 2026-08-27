#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGES="$ROOT/.tools/packages"
SYSROOT="$ROOT/.tools/sysroot"
mkdir -p "$PACKAGES" "$SYSROOT"

fetch() {
  local name="$1"
  local url="$2"
  local archive="$PACKAGES/$name.pkg.tar.zst"
  if [[ ! -f "$archive" ]]; then
    curl -L --fail --silent --show-error -o "$archive" "$url"
  fi
  bsdtar -xf "$archive" -C "$SYSROOT"
}

fetch at-spi2-core https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/at-spi2-core-2.60.5-1-x86_64.pkg.tar.zst
fetch libxcomposite https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/libxcomposite-0.4.7-1-x86_64.pkg.tar.zst
fetch libxdamage https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/libxdamage-1.1.7-1-x86_64.pkg.tar.zst
fetch libxrandr https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/libxrandr-1.5.5-1-x86_64.pkg.tar.zst
fetch alsa-lib https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/alsa-lib-1.2.16.1-1-x86_64.pkg.tar.zst
fetch xorg-server-xvfb https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/xorg-server-xvfb-21.1.24-1-x86_64.pkg.tar.zst
fetch xorg-server-common https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/xorg-server-common-21.1.24-1-x86_64.pkg.tar.zst
fetch libxcvt https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/libxcvt-0.1.3-1-x86_64.pkg.tar.zst
fetch libxfont2 https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/libxfont2-2.0.8-1-x86_64.pkg.tar.zst
fetch libfontenc https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/libfontenc-1.1.9-1-x86_64.pkg.tar.zst
fetch xorg-xkbcomp https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/xorg-xkbcomp-1.5.0-1-x86_64.pkg.tar.zst
fetch xdotool https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/xdotool-4.20260303.1-1-x86_64.pkg.tar.zst
fetch libxtst https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/libxtst-1.2.5-1-x86_64.pkg.tar.zst
fetch libxinerama https://au.mirrors.cicku.me/archlinux/extra/os/x86_64/libxinerama-1.1.6-1-x86_64.pkg.tar.zst

ln -sf /opt/bash "$SYSROOT/usr/bin/sh"
