#!/bin/sh
# Removes what install.sh added.
#
#   ./uninstall.sh                 if you installed it just for you
#   sudo ./uninstall.sh --system   if you installed it for everyone
#
# Your settings, log and sign-in data stay in ~/.config/ipabazooka.
# Delete that folder too if you want to remove everything.
set -eu

if [ "${1:-}" = "--system" ]; then
    if [ "$(id -u)" -ne 0 ]; then
        echo "Run:  sudo $0 --system" >&2
        exit 1
    fi
    bindir=/usr/local/bin
    datadir=/usr/local/share
else
    bindir="$HOME/.local/bin"
    datadir="${XDG_DATA_HOME:-$HOME/.local/share}"
fi

rm -f "$bindir/ipabazooka" \
      "$datadir/applications/ipabazooka.desktop" \
      "$datadir/mime/packages/ipabazooka.xml"
for size in 16 32 48 64 128 256 512; do
    rm -f "$datadir/icons/hicolor/${size}x${size}/apps/ipabazooka.png"
done

command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$datadir/mime" >/dev/null 2>&1 || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$datadir/applications" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t -f "$datadir/icons/hicolor" >/dev/null 2>&1 || true

echo "ipabazooka was removed. Your settings are still in ~/.config/ipabazooka (delete that folder to remove them too)."
