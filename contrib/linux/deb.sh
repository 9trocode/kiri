#!/bin/sh
# Builds target/kiri_<version>_<arch>.deb on a Debian or Ubuntu machine:
# the engine and CLI (/usr/bin/kiri, kiri-init), the kiri system service,
# and the window (/usr/lib/kiri/kiri-window) with its launcher and icon.
#
# Needs: Rust, dpkg-deb, and libwebkit2gtk-4.1-dev for the window. Run it
# on the oldest release the package should install on: the window links
# that release's glibc, GTK and WebKitGTK.
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
arch=$(dpkg --print-architecture)

cargo build --release -p kiri -p kiri-init
# Its own target dir: the same crate with another feature set would
# otherwise rebuild over the engine's binary each time.
CARGO_TARGET_DIR=target/window cargo build --release -p kiri --features window

pkg=target/deb/kiri_${version}_${arch}
rm -rf "$pkg"
install -d "$pkg/DEBIAN" "$pkg/usr/bin" "$pkg/usr/lib/kiri" "$pkg/usr/lib/systemd/system" "$pkg/usr/share/applications"
install -m 755 target/release/kiri target/release/kiri-init "$pkg/usr/bin/"
install -m 755 target/window/release/kiri "$pkg/usr/lib/kiri/kiri-window"
sed 's|^ExecStart=/usr/local/bin/kiri daemon|ExecStart=/usr/bin/kiri daemon|' contrib/linux/kiri.service >"$pkg/usr/lib/systemd/system/kiri.service"
install -m 644 contrib/linux/kiri.desktop "$pkg/usr/share/applications/kiri.desktop"
for s in 16 32 64 128 256 512; do
    install -D -m 644 "brand/icon/png/kiri-app-icon-$s.png" "$pkg/usr/share/icons/hicolor/${s}x${s}/apps/kiri.png"
done
install -D -m 644 brand/icon/kiri-app-icon.svg "$pkg/usr/share/icons/hicolor/scalable/apps/kiri.svg"
install -m 755 contrib/linux/deb/postinst contrib/linux/deb/prerm contrib/linux/deb/postrm "$pkg/DEBIAN/"

size=$(du -sk "$pkg/usr" | cut -f1)
cat >"$pkg/DEBIAN/control" <<EOF
Package: kiri
Version: $version
Architecture: $arch
Maintainer: Kiri
Homepage: https://runkiri.com
Section: admin
Priority: optional
Installed-Size: $size
Depends: libc6, libgtk-3-0t64 | libgtk-3-0, libwebkit2gtk-4.1-0, docker.io | docker-ce-cli
Recommends: docker-buildx | docker-buildx-plugin, docker-compose-v2 | docker-compose-plugin
Description: Run, build and share containers
 Kiri is a small container engine with its own window. Plain docker
 commands work against it through the "kiri" docker context, and the
 window shows containers, images, volumes, builds and Kubernetes, and
 shares a container on a public https link.
EOF
dpkg-deb --root-owner-group --build "$pkg" "target/kiri_${version}_${arch}.deb"
echo "$root/target/kiri_${version}_${arch}.deb"
