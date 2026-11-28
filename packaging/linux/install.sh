#!/bin/sh
# Installs ipabazooka from this folder and adds it to your app menu.
#
#   ./install.sh                  just for you (into ~/.local, no password needed)
#   sudo ./install.sh --system    for everyone on this computer (into /usr/local)
#
# Remove it again with ./uninstall.sh (or sudo ./uninstall.sh --system).
set -eu

here=$(cd "$(dirname "$0")" && pwd)

if [ "${1:-}" = "--system" ]; then
    if [ "$(id -u)" -ne 0 ]; then
        echo "Installing for everyone needs administrator rights. Run:  sudo $0 --system" >&2
        exit 1
    fi
    bindir=/usr/local/bin
    datadir=/usr/local/share
else
    bindir="$HOME/.local/bin"
    datadir="${XDG_DATA_HOME:-$HOME/.local/share}"
fi

echo "Installing ipabazooka into $bindir ..."
install -Dm755 "$here/ipabazooka" "$bindir/ipabazooka"
for size in 16 32 48 64 128 256 512; do
    install -Dm644 "$here/icons/ipabazooka-$size.png" "$datadir/icons/hicolor/${size}x${size}/apps/ipabazooka.png"
done
install -Dm644 "$here/ipabazooka-mime.xml" "$datadir/mime/packages/ipabazooka.xml"
# point the menu entry at the exact file, in case $bindir isn't on PATH yet
mkdir -p "$datadir/applications"
sed "s|^Exec=ipabazooka|Exec=\"$bindir/ipabazooka\"|" "$here/ipabazooka.desktop" > "$datadir/applications/ipabazooka.desktop"
chmod 644 "$datadir/applications/ipabazooka.desktop"

# refresh the desktop's caches (each is optional)
command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$datadir/mime" >/dev/null 2>&1 || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$datadir/applications" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t -f "$datadir/icons/hicolor" >/dev/null 2>&1 || true

echo "Done. Open ipabazooka from your app menu, or run: $bindir/ipabazooka"

# usbmuxd is what lets Linux talk to an iPhone or iPad
found=no
for p in /usr/sbin/usbmuxd /usr/bin/usbmuxd /sbin/usbmuxd /usr/local/sbin/usbmuxd /usr/local/bin/usbmuxd /run/current-system/sw/bin/usbmuxd; do
    [ -e "$p" ] && found=yes
done
if [ "$found" = no ]; then
    id=""; like=""
    if [ -r /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        id="${ID:-}"; like="${ID_LIKE:-}"
    fi
    case " $id $like " in
        *" debian "*|*" ubuntu "*) cmd="sudo apt install usbmuxd" ;;
        # RHEL and its rebuilds (whose ID_LIKE also says fedora) get usbmuxd from EPEL
        *" rhel "*|*" centos "*) cmd="sudo dnf install epel-release, then: sudo dnf install usbmuxd" ;;
        *" fedora "*) cmd="sudo dnf install usbmuxd" ;;
        *" suse "*|*" opensuse "*) cmd="sudo zypper install usbmuxd" ;;
        *" arch "*) cmd="sudo pacman -S usbmuxd" ;;
        *" void "*) cmd="sudo xbps-install usbmuxd" ;;
        *) cmd="install the usbmuxd package with your package manager" ;;
    esac
    echo
    echo "One more thing: usbmuxd isn't installed. It lets Linux talk to your iPhone or iPad."
    echo "Install it with:  $cmd"
fi
