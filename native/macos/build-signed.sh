#!/bin/sh
set -eu
# Run on a Mac with Xcode tools. Does not install launch jobs or change permissions.
if [ "$#" -ne 2 ]; then echo "usage: $0 'Developer ID Application: ...' TEAM_ID" >&2; exit 1; fi
case "$2" in *[!A-Z0-9]*|'') echo "Invalid team identifier" >&2; exit 1;; esac
cd "$(dirname "$0")"
swift build -c release
mkdir -p .build/signed
for role in hostd agent capture-helper; do
  cp .build/release/dh-macos-probe ".build/signed/dh-$role"
  codesign --force --options runtime --timestamp --sign "$1" --identifier "org.deskharbor.p0.$role" ".build/signed/dh-$role"
  codesign --verify --strict --verbose=2 -R="anchor apple generic and certificate leaf[subject.OU] = \"$2\"" ".build/signed/dh-$role"
done
echo 'Signed probe executables ready. No installation or LoginWindow proof has run.'
