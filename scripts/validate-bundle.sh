#!/bin/bash
set -euo pipefail

bundle="${1:-target/clap/MKS-7 Controller.clap}"
executable="$bundle/Contents/MacOS/MKS-7 Controller"
plist="$bundle/Contents/Info.plist"
dist="$(dirname "$bundle")"

test -f "$executable"
test -f "$plist"
test -f "$dist/LICENSE"
test -f "$dist/README.md"
test -f "$dist/THIRD_PARTY_LICENSES.html"
test "$(lipo -archs "$executable")" = "arm64"

build_info="$(vtool -arch arm64 -show-build "$executable")"
case "$build_info" in
    *"minos 11.0"*) ;;
    *) echo "Expected macOS 11.0 deployment target" >&2; exit 1 ;;
esac

plutil -lint "$plist"
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$plist")" = "MKS-7 Controller"
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist")" = "com.marcellkovacs.mks7-controller"
package_version="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "mks7_clap_controller") | .version')"
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist")" = "$package_version"
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$plist")" = "$package_version"

codesign --verify --deep --strict --verbose=2 "$bundle"
clap-validator validate "$bundle" --only-failed
