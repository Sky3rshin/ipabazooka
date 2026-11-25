#!/bin/sh
# Builds the Linux downloads from a compiled ipabazooka binary:
#   ipabazooka-<v>-linux-x86_64.tar.gz   portable folder with install.sh
#   ipabazooka_<v>_amd64.deb             Debian, Ubuntu, Mint, Pop!_OS ...
#   ipabazooka-<v>-1.x86_64.rpm          Fedora, RHEL/Alma/Rocky, openSUSE
#   ipabazooka-<v>-x86_64.AppImage       one file for any distribution
#   PKGBUILD                             Arch recipe (the .pkg.tar.zst is made
#                                        from it on Arch by makepkg)
#
# Usage: packaging/linux/build-packages.sh <path/to/ipabazooka> <output-dir>
# Needs dpkg-deb and rpmbuild. appimagetool is downloaded unless
# APPIMAGETOOL points at a copy.
set -eu

bin=$1
out=$2
src=$(cd "$(dirname "$0")" && pwd)
top=$(cd "$src/../.." && pwd)
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$top/Cargo.toml" | head -n 1)
arch=x86_64
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"
out=$(cd "$out" && pwd)
metainfo=io.github.sky3rshin.ipabazooka.metainfo.xml

echo "ipabazooka $version: packaging $bin"
fill() { sed -e "s/@VERSION@/$version/g" "$1"; }

# ---------- the installed file tree (/usr/...), shared by .deb and .rpm ----------
tree="$work/root"
install -Dm755 "$bin" "$tree/usr/bin/ipabazooka"
install -Dm644 "$src/ipabazooka.desktop" "$tree/usr/share/applications/ipabazooka.desktop"
for s in 16 32 48 64 128 256 512; do
    install -Dm644 "$src/icons/ipabazooka-$s.png" "$tree/usr/share/icons/hicolor/${s}x${s}/apps/ipabazooka.png"
done
install -Dm644 "$src/ipabazooka-mime.xml" "$tree/usr/share/mime/packages/ipabazooka.xml"
install -Dm644 "$src/$metainfo" "$tree/usr/share/metainfo/$metainfo"
install -Dm644 "$src/COPYRIGHT.txt" "$tree/usr/share/doc/ipabazooka/copyright"
fill "$src/README-LINUX.txt" > "$tree/usr/share/doc/ipabazooka/README-LINUX.txt"
chmod 644 "$tree/usr/share/doc/ipabazooka/README-LINUX.txt"

# ---------- 1. portable folder (.tar.gz) ----------
name="ipabazooka-$version-linux-$arch"
p="$work/$name"
mkdir -p "$p/icons"
install -m755 "$bin" "$p/ipabazooka"
install -m644 "$src"/icons/ipabazooka-*.png "$p/icons/"
install -m644 "$src/ipabazooka.desktop" "$src/ipabazooka-mime.xml" "$src/$metainfo" "$src/COPYRIGHT.txt" "$p/"
fill "$src/README-LINUX.txt" > "$p/README-LINUX.txt"
chmod 644 "$p/README-LINUX.txt"
install -m755 "$src/install.sh" "$src/uninstall.sh" "$p/"
tar -C "$work" --owner=0 --group=0 --numeric-owner -czf "$out/$name.tar.gz" "$name"
echo "built $name.tar.gz"

# ---------- 2. .deb ----------
d="$work/deb"
cp -a "$tree" "$d"
mkdir -p "$d/DEBIAN"
size=$(du -sk "$d" | cut -f1)
cat > "$d/DEBIAN/control" <<EOF
Package: ipabazooka
Version: $version
Architecture: amd64
Maintainer: CyberWithSkyer <355110127+Sky3rshin@users.noreply.github.com>
Installed-Size: $size
Depends: usbmuxd, libc6 (>= 2.28), libgcc-s1 | libgcc1, libgl1, libegl1, libxkbcommon0, libxkbcommon-x11-0, libx11-6, libxcursor1, libxrandr2, libxi6, libwayland-client0, libwayland-cursor0, libwayland-egl1
Recommends: xdg-desktop-portal | zenity
Suggests: libimobiledevice-utils
Section: utils
Priority: optional
Homepage: https://github.com/Sky3rshin/ipabazooka
Description: Install apps (.ipa files) on your iPhone or iPad
 ipabazooka is a beginner-friendly Sideloadly alternative. It signs an app
 file (.ipa) with your own free Apple ID and installs it on an iPhone or iPad
 over USB.
 .
 It explains every step, shows errors in plain English and comes with
 23 themes.
EOF
# xz (not zstd) so older dpkg versions, like Debian 11's, can open it too
dpkg-deb -Zxz --root-owner-group --build "$d" "$out/ipabazooka_${version}_amd64.deb"
echo "built ipabazooka_${version}_amd64.deb"

# ---------- 3. .rpm ----------
r="$work/rpmbuild"
mkdir -p "$r/SOURCES" "$r/SPECS"
cp -a "$tree" "$r/SOURCES/root"
fill "$src/ipabazooka.spec.in" > "$r/SPECS/ipabazooka.spec"
rpmbuild -bb --quiet \
    --define "_topdir $r" \
    --define "_binary_payload w9.xzdio" \
    "$r/SPECS/ipabazooka.spec"
cp "$r/RPMS/$arch/ipabazooka-$version-1.$arch.rpm" "$out/"
echo "built ipabazooka-$version-1.$arch.rpm"

# ---------- 4. AppImage ----------
a="$work/AppDir"
install -Dm755 "$bin" "$a/usr/bin/ipabazooka"
for s in 16 32 48 64 128 256 512; do
    install -Dm644 "$src/icons/ipabazooka-$s.png" "$a/usr/share/icons/hicolor/${s}x${s}/apps/ipabazooka.png"
done
install -Dm644 "$src/ipabazooka.desktop" "$a/ipabazooka.desktop"
install -Dm644 "$src/ipabazooka.desktop" "$a/usr/share/applications/ipabazooka.desktop"
install -Dm644 "$src/icons/ipabazooka-256.png" "$a/ipabazooka.png"
ln -s ipabazooka.png "$a/.DirIcon"
cat > "$a/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/ipabazooka" "$@"
EOF
chmod 755 "$a/AppRun"
tool=${APPIMAGETOOL:-}
if [ -z "$tool" ]; then
    tool="$work/appimagetool"
    curl -fsSL -o "$tool" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
    chmod +x "$tool"
fi
ARCH=$arch APPIMAGE_EXTRACT_AND_RUN=1 "$tool" "$a" "$out/ipabazooka-$version-$arch.AppImage"
chmod 755 "$out/ipabazooka-$version-$arch.AppImage"
echo "built ipabazooka-$version-$arch.AppImage"

# ---------- 5. Arch PKGBUILD (points at the portable archive above) ----------
sum=$(sha256sum "$out/$name.tar.gz" | cut -d' ' -f1)
fill "$src/PKGBUILD.in" | sed -e "s/@SHA256@/$sum/" > "$out/PKGBUILD"
echo "built PKGBUILD"

ls -l "$out"
