# DeskHarbor client

Flutter3.47.9/Dart3.13.7, Riverpod3.5.0, go_router18.1.3 and flutter_rust_bridge2.13.0 are pinned. All four native runners are generated from the pinned SDK.

The Material3 client uses the actual Rust DeskEngine through packages/engine_bridge. Test fixtures live only in test/support. Native textures carry handles and frame metadata through Dart; pixels remain in Rust/native renderer memory. Missing engine, permission and video capabilities are displayed as unavailable.

## Build order

1. Build libvpx for the target and configure VPX_INCLUDE_DIR, VPX_LIB_DIR and, where needed, VPX_LIB_NAME. Build cargo build --locked --release -p dh-ffi at the repository root for the matching runner architecture.
2. Generate the bridge as documented in packages/engine_bridge/README.md. Commit generated Rust and Dart outputs together.
3. Run flutter pub get, flutter analyze --no-pub and flutter test --no-pub in this directory.
4. Build flutter build windows, flutter build linux, flutter build macos or flutter build apk --target-platform android-arm64 on a suitable host.

Windows/Linux CMake and the macOS embed script use repository target/release or the process variable DESKHARBOR_RUST_LIBRARY. Windows packages dh_ffi.dll beside the executable; Linux packages libdh_ffi.so under bundle/lib; macOS embeds libdh_ffi.dylib in Contents/Frameworks, updates its install name and signs it when Xcode signing is enabled. Universal macOS builds require a universal Rust library. Ad hoc build signatures do not provide Developer ID trust or LoginWindow qualification.

Android uses SDK36/NDK28.2.13676358, minSdk30 and ARM64. Build Rust for aarch64-linux-android and copy libdh_ffi.so into android/app/src/main/jniLibs/arm64-v8a before Flutter builds. Native library binaries are ignored in Git. Release signing requires DESKHARBOR_KEYSTORE_PATH, DESKHARBOR_KEYSTORE_PASSWORD, DESKHARBOR_KEY_ALIAS and DESKHARBOR_KEY_PASSWORD; release never uses a debug key. Keep secrets outside the repository.

## Verification limits

This Windows workstation lacks Flutter desktop CMake integration, and plugin symlinks require Developer Mode. Global Flutter, Visual Studio and Windows configuration are left unchanged. Hosted Windows/Ubuntu/macOS runners can build using the pinned SDK/native dependencies. Linux additionally needs GTK3 development headers. Android requires the pinned NDK and target Rust library.

Compilation and fixture tests do not qualify native capture/input, secure desktop, Android consent, publisher trust or production release gates.
