#!/bin/sh
# Compiled with swiftc, not as a Swift package: `swift build` stamps the
# binary with its deployment target as the SDK it was built with, and macOS
# then draws the app's buttons and fields as that older system did.
set -eu
cd "$(dirname "$0")"
app=${1:-build}/Clauset.app
archs=${ARCHS:-$(uname -m)}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' ../Cargo.toml | head -1)
# Sparkle's release, held to the hash of the archive this version was written
# against.
sparkle=build/Sparkle-2.10.1
if [ ! -d "$sparkle/Sparkle.framework" ]; then
  mkdir -p "$sparkle"
  curl -fsSL -o "$sparkle.tar.xz" \
    https://github.com/sparkle-project/Sparkle/releases/download/2.10.1/Sparkle-2.10.1.tar.xz
  echo "1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27  $sparkle.tar.xz" |
    shasum -a 256 -c - >/dev/null
  tar -xJf "$sparkle.tar.xz" -C "$sparkle"
  rm "$sparkle.tar.xz"
fi
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Frameworks"
slices=
for arch in $archs; do
  xcrun swiftc -O -parse-as-library -swift-version 6 \
    -target "$arch-apple-macos14.4" -module-name Clauset \
    -F "$sparkle" -framework Sparkle -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
    Sources/*.swift -o "build/Clauset-$arch"
  slices="$slices build/Clauset-$arch"
done
lipo -create $slices -output "$app/Contents/MacOS/Clauset"
rm $slices
# The framework as it ships, its own ad hoc signatures included, without
# what only a compiler reads.
ditto "$sparkle/Sparkle.framework" "$app/Contents/Frameworks/Sparkle.framework"
(cd "$app/Contents/Frameworks/Sparkle.framework" &&
  rm -rf Headers PrivateHeaders Modules Versions/B/Headers Versions/B/PrivateHeaders Versions/B/Modules)
# Assets.car carries the Liquid Glass icon for macOS 26 and later,
# Clauset.icns the one for 14.4 and 15.
xcrun actool Clauset.icon --compile "$app/Contents/Resources" --app-icon Clauset \
  --enable-on-demand-resources NO --development-region en --target-device mac \
  --platform macosx --minimum-deployment-target 14.4 \
  --output-partial-info-plist build/icon.plist >/dev/null
cp Info.plist "$app/Contents/"
plutil -insert CFBundleShortVersionString -string "$version" "$app/Contents/Info.plist"
plutil -insert CFBundleVersion -string "$version" "$app/Contents/Info.plist"
codesign --force --sign - "$app/Contents/Frameworks/Sparkle.framework"
codesign --force --sign - "$app"
echo "$(pwd)/$app"
