#!/usr/bin/env bash
# Packages a release build into dist/ for one system.
#
#   scripts/package-release.sh linux   <version>        # built for the host
#   scripts/package-release.sh windows <version>
#   scripts/package-release.sh macos   <version> <MoltenVK dylib>
#
# Linux and Windows read target/release; macOS reads
# target/{aarch64,x86_64}-apple-darwin/release and joins them into one
# universal application. Every archive holds both programs: the app starts
# the document server from its own folder.
set -euo pipefail

system="$1"
version="$2"
root="$(cd "$(dirname "$0")/.." && pwd)"
dist="$root/dist"
mkdir -p "$dist"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

docs=(README.md LICENSE-MIT LICENSE-APACHE)

case "$system" in
linux)
  name="printcad-$version-linux-x86_64"
  dir="$stage/$name"
  mkdir -p "$dir"
  install -m 755 "$root/target/release/printcad" "$root/target/release/printcad-serverd" "$dir/"
  for doc in "${docs[@]}"; do cp "$root/$doc" "$dir/"; done
  cat > "$dir/printcad.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=printCAD
Comment=Parametric CAD for 3D printing
Exec=printcad %F
Terminal=false
Categories=Graphics;Engineering;3DGraphics;
MimeType=model/step;model/stl;model/3mf;
EOF
  tar -C "$stage" -czf "$dist/$name.tar.gz" "$name"
  echo "$dist/$name.tar.gz"
  ;;

windows)
  name="printcad-$version-windows-x86_64"
  dir="$stage/$name"
  mkdir -p "$dir"
  cp "$root/target/release/printcad.exe" "$root/target/release/printcad-serverd.exe" "$dir/"
  for doc in "${docs[@]}"; do cp "$root/$doc" "$dir/"; done
  (cd "$stage" && 7z a -tzip -bso0 "$dist/$name.zip" "$name")
  echo "$dist/$name.zip"
  ;;

macos)
  moltenvk="$3"
  name="printcad-$version-macos-universal"
  app="$stage/dmg/printCAD.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources"
  for program in printcad printcad-serverd; do
    lipo -create \
      "$root/target/aarch64-apple-darwin/release/$program" \
      "$root/target/x86_64-apple-darwin/release/$program" \
      -output "$app/Contents/MacOS/$program"
  done
  # The renderer loads Vulkan from here when the system has none.
  cp "$moltenvk" "$app/Contents/Frameworks/libMoltenVK.dylib"
  for doc in "${docs[@]}"; do cp "$root/$doc" "$app/Contents/Resources/"; done
  cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>printcad</string>
  <key>CFBundleIdentifier</key><string>io.github.gilbertorconde.printcad</string>
  <key>CFBundleName</key><string>printCAD</string>
  <key>CFBundleDisplayName</key><string>printCAD</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
  # Apple silicon runs nothing unsigned; an ad-hoc signature is enough to
  # run once the download is let through.
  codesign --force --deep --sign - "$app"
  ln -s /Applications "$stage/dmg/Applications"
  hdiutil create -volname printCAD -srcfolder "$stage/dmg" -format UDZO -ov "$dist/$name.dmg"
  echo "$dist/$name.dmg"
  ;;

*)
  echo "unknown system: $system" >&2
  exit 2
  ;;
esac
