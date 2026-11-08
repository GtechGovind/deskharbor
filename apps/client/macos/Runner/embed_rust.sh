#!/bin/sh
set -eu
library="${DESKHARBOR_RUST_LIBRARY:-$PROJECT_DIR/../../../target/release/libdh_ffi.dylib}"
if [ ! -f "$library" ]; then
  echo "error: Build dh-ffi --release for the requested macOS architectures first, or set DESKHARBOR_RUST_LIBRARY." >&2
  exit 1
fi
destination="$TARGET_BUILD_DIR/$FRAMEWORKS_FOLDER_PATH"
mkdir -p "$destination"
cp "$library" "$destination/libdh_ffi.dylib"
install_name_tool -id '@rpath/libdh_ffi.dylib' "$destination/libdh_ffi.dylib"
if [ "${CODE_SIGNING_ALLOWED:-NO}" = YES ]; then
  codesign --force --sign "${EXPANDED_CODE_SIGN_IDENTITY:--}" "$destination/libdh_ffi.dylib"
fi
