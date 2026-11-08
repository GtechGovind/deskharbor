# Native video surfaces

Use `NativeSurface.create(sessionId)`, then `present()` for each Rust FrameReady event, and `Texture(textureId: surface.textureId)`. Dispose when leaving the session. Only identifiers cross the method channel; decoded bytes remain native. The UI must hide/unmount the texture on `frame_unavailable`, `library_unavailable`, or any surface error to avoid showing a stale last frame. Creation registers an empty texture; it does not assert that a decoded frame exists.

The application must initialize its FRB engine once before creating surfaces. Package the **same dynamically loaded dh_ffi library**, including the frame C ABI, beside the Windows runner (`dh_ffi.dll`), in Linux `bundle/lib` (`libdh_ffi.so`), in macOS `Contents/Frameworks` with the runner's framework rpath (`libdh_ffi.dylib`), or Android `jniLibs/<ABI>` (`libdh_ffi.so`). Do not statically link another engine into this plugin. The plugin holds its module handle until process exit so outstanding renderer leases can safely release.

Windows uses PixelBufferTexture and returns leased tightly packed RGBA pixels. Flutter completion atomically releases the lease; descriptors stay allocated until asynchronous unregister completion. Linux FlPixelBufferTexture copies to renderer-owned RGBA storage and releases the Rust source after the synchronous copy. macOS converts to a retained BGRA CVPixelBuffer, releasing the Rust source after copying. Android SurfaceProducer retrieves the current Surface for each draw, copies RGBA with ANativeWindow destination row stride, posts the native buffer, and releases its source and window on every path. All use top-left row order without a vertical flip. Android pauses while its surface is cleaned up, releases producers on detach, targets API24+, and pins NDK28.2.13676358. Rust libraries must match every runner ABI (including Android arm64-v8a and x86_64); 32-bit architectures require a matching engine library too.

`common/dh_frame.h` defines the C ABI and caller obligations. No native plugin creates an engine. Missing ABI symbols return `library_unavailable`; missing decoded frames return `frame_unavailable`. Native output failures return an explicit error instead of a generated success frame.

## Verification

- `flutter test` and `flutter analyze` from this package.
- `rustc --edition=2024 --test packages/native_surfaces/tests/rust/lease_harness.rs -o <scratch-exe>` from the repository, then execute it. This includes the actual FFI helper implementation with only the engine/DecodedFrame dependency boundary supplied by the harness. `cargo test -p dh-ffi` runs the same tests against the real engine/media crates.
- Compile and run `tests/native/copy_test.cpp` and `tests/native/lease_test.cpp` with C++17, warnings as errors, and AddressSanitizer. They exercise the exact pixel-copy and atomic-release helpers used by the plugins; corners detect RGBA/BGRA mistakes and row inversion, while the lease test exercises two racing completion calls and shutdown release.
- Windows plugin can be checked with MSVC `/Zs /std:c++17 /W4 /WX /EHsc /DNATIVE_SURFACES_PLUGIN_IMPL` against the Flutter SDK engine and cpp_client_wrapper include directories.

A passing helper test or source compilation is not a displayed-video proof. Each packaged runner still requires a real Flutter texture upload, resize, unavailable-frame, background/resume, and detach smoke test. Do not use an actual user screen or input injection for these tests; feed deliberately labelled corner fixtures.
