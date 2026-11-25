#!/bin/sh
# Installs one of the Linux downloads on a fresh distribution (used by CI) and
# checks that it works:
#   * the package installs with the system's own package manager, which also
#     pulls in its dependencies (usbmuxd, graphics libraries)
#   * the program, menu entry, icons and .ipa file type are in place
#   * every library the program links to is found
#   * the app opens and draws its window on a virtual screen (saved as a PNG)
#   * its data folder is private to the user
#
# Usage: ci-test.sh <deb|rpm|suse|arch|appimage|tarball> <packages-dir> <screenshot-dir>
set -eux

kind=$1
pkgs=$(cd "$2" && pwd)
mkdir -p "$3"
shots=$(cd "$3" && pwd)
. /etc/os-release
distro="${ID}-${VERSION_ID:-rolling}"
system_install=yes

case "$kind" in
deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y "$pkgs"/ipabazooka_*_amd64.deb
    apt-get install -y xvfb xauth libgl1-mesa-dri desktop-file-utils
    exe=/usr/bin/ipabazooka
    theme=normal
    ;;
rpm)
    dnf install -y "$pkgs"/ipabazooka-*.x86_64.rpm
    dnf install -y xorg-x11-server-Xvfb xorg-x11-xauth mesa-dri-drivers desktop-file-utils
    exe=/usr/bin/ipabazooka
    theme=catppuccin-mocha
    ;;
suse)
    zypper --non-interactive refresh
    zypper --non-interactive install --allow-unsigned-rpm "$pkgs"/ipabazooka-*.x86_64.rpm
    zypper --non-interactive install xvfb-run xauth Mesa-dri desktop-file-utils
    exe=/usr/bin/ipabazooka
    theme=everforest
    ;;
arch)
    pacman -Syu --noconfirm
    pacman -U --noconfirm "$pkgs"/ipabazooka-*-x86_64.pkg.tar.zst
    pacman -S --noconfirm --needed xorg-server-xvfb xorg-xauth mesa desktop-file-utils
    exe=/usr/bin/ipabazooka
    theme=tokyo-night
    ;;
appimage)
    # an AppImage brings the program; the graphics libraries come from the system
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y xvfb xauth libgl1 libegl1 libgl1-mesa-dri libxkbcommon0 libxkbcommon-x11-0 \
        libx11-6 libxcursor1 libxrandr2 libxi6 libwayland-client0 libwayland-cursor0 libwayland-egl1
    cp "$pkgs"/ipabazooka-*-x86_64.AppImage /tmp/ipabazooka.AppImage
    chmod +x /tmp/ipabazooka.AppImage
    # containers have no FUSE; this makes the AppImage unpack itself and run
    export APPIMAGE_EXTRACT_AND_RUN=1
    exe=/tmp/ipabazooka.AppImage
    theme=hacker
    system_install=no
    ;;
tarball)
    dnf install -y xorg-x11-server-Xvfb xorg-x11-xauth mesa-dri-drivers mesa-libGL mesa-libEGL \
        libxkbcommon libxkbcommon-x11 libX11 libXcursor libXrandr libXi \
        libwayland-client libwayland-cursor libwayland-egl tar gzip findutils desktop-file-utils
    tar -C /tmp -xzf "$pkgs"/ipabazooka-*-linux-x86_64.tar.gz
    sh /tmp/ipabazooka-*-linux-x86_64/install.sh
    exe="$HOME/.local/bin/ipabazooka"
    test -x "$exe"
    test -f "$HOME/.local/share/applications/ipabazooka.desktop"
    test -f "$HOME/.local/share/icons/hicolor/256x256/apps/ipabazooka.png"
    test -f "$HOME/.local/share/mime/packages/ipabazooka.xml"
    desktop-file-validate "$HOME/.local/share/applications/ipabazooka.desktop"
    theme=synthwave
    system_install=no
    ;;
*)
    echo "unknown kind: $kind" >&2
    exit 2
    ;;
esac

if [ "$system_install" = yes ]; then
    test -x /usr/bin/ipabazooka
    test -f /usr/share/applications/ipabazooka.desktop
    desktop-file-validate /usr/share/applications/ipabazooka.desktop
    for s in 16 32 48 64 128 256 512; do
        test -f "/usr/share/icons/hicolor/${s}x${s}/apps/ipabazooka.png"
    done
    test -f /usr/share/mime/packages/ipabazooka.xml
    test -f /usr/share/metainfo/io.github.sky3rshin.ipabazooka.metainfo.xml
    # usbmuxd came along as a dependency
    command -v usbmuxd || ls /usr/sbin/usbmuxd /usr/bin/usbmuxd 2>/dev/null
fi

if [ "$kind" != appimage ]; then
    ldd "$exe" | tee /tmp/ldd.txt
    if grep -q 'not found' /tmp/ldd.txt; then
        echo "missing libraries" >&2
        exit 1
    fi
fi

# open the app on a virtual screen; it saves a picture of its window and quits
export XDG_CONFIG_HOME=/tmp/ipabazooka-config
shot="$shots/$distro-$kind.png"
timeout 180 xvfb-run -a -s "-screen 0 1280x1024x24" "$exe" --screenshot "$shot" --theme "$theme"
test -s "$shot"
bytes=$(wc -c < "$shot")
test "$bytes" -gt 20000

# the data folder holds the signing key, so it must be private
perm=$(stat -c %a "$XDG_CONFIG_HOME/ipabazooka")
test "$perm" = 700

echo "OK: $kind on $PRETTY_NAME ($bytes-byte screenshot)"
