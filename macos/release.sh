#!/bin/sh
# Needs the Rust target x86_64-apple-darwin, the Developer ID identity and
# Sparkle's signing key in the login keychain, and the notarytool profile
# `clauset`. Notarizing sends the build to Apple.
set -eu
cd "$(dirname "$0")"
identity="Developer ID Application: Hold My Beer B.V. (JHSKH29BHP)"
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' ../Cargo.toml | head -1)
out=build/release
app=$out/Clauset.app
sparkle=$app/Contents/Frameworks/Sparkle.framework/Versions/B

rm -rf "$out"
mkdir -p "$out"

export MACOSX_DEPLOYMENT_TARGET=14.4
(cd .. && cargo build --release --locked --target aarch64-apple-darwin --target x86_64-apple-darwin)
ARCHS="arm64 x86_64" ./bundle.sh "$out" >/dev/null
mkdir -p "$app/Contents/Helpers"
lipo -create ../target/aarch64-apple-darwin/release/clauset ../target/x86_64-apple-darwin/release/clauset \
  -output "$app/Contents/Helpers/clauset"

# Innermost first: a signature covers what lies under it. Sparkle's
# downloader keeps the entitlements it ships with.
sign() { codesign --force --options runtime --timestamp --sign "$identity" "$@"; }
sign "$sparkle/XPCServices/Installer.xpc"
sign --preserve-metadata=entitlements "$sparkle/XPCServices/Downloader.xpc"
sign "$sparkle/Autoupdate"
sign "$sparkle/Updater.app"
sign "$app/Contents/Frameworks/Sparkle.framework"
sign "$app/Contents/Helpers/clauset"
sign "$app"
codesign --verify --deep --strict "$app"

notarize() { xcrun notarytool submit "$1" --keychain-profile clauset --wait; }
ditto -c -k --keepParent "$app" "$out/Clauset.zip"
notarize "$out/Clauset.zip"
rm "$out/Clauset.zip"
xcrun stapler staple "$app"

mkdir "$out/image"
ditto "$app" "$out/image/Clauset.app"
ln -s /Applications "$out/image/Applications"
hdiutil create -quiet -volname Clauset -srcfolder "$out/image" -format UDZO "$out/Clauset.dmg"
rm -rf "$out/image"
sign "$out/Clauset.dmg"
notarize "$out/Clauset.dmg"
xcrun stapler staple "$out/Clauset.dmg"

mkdir "$out/feed"
cp "$out/Clauset.dmg" "$out/feed/"
build/Sparkle-2.10.1/bin/generate_appcast "$out/feed" \
  --download-url-prefix "https://github.com/mischasigtermans/clauset/releases/download/v$version/" \
  --link https://clauset.app \
  --full-release-notes-url https://clauset.app/changelog/ \
  -o "$out/appcast.xml"
rm -rf "$out/feed"
echo "$(pwd)/$out/Clauset.dmg"
echo "$(pwd)/$out/appcast.xml"
