# Engine bridge

`flutter_rust_bridge_codegen` and the Dart/Rust runtimes are pinned to **2.13.0**.
From this directory, with Flutter and workspace-local cargo-expand on the process
PATH, run `flutter_rust_bridge_codegen generate --config-file flutter_rust_bridge.yaml`.
The Rust native VP8 include/library environment must be set as described in the
repository build documentation. Generated files must be regenerated together.
After generation, run the pinned Rust formatter from the repository root:
`rustfmt --edition 2024 --config skip_children=true crates/dh-ffi/src/lib.rs crates/dh-ffi/src/frb_generated.rs`.

`EngineGateway` is the injectable control boundary. `FrbEngine` is the sole
production implementation and calls the generated bridge. No Dart method carries
pixels. Frame events contain only a session ID, sequence and dimensions; native
texture plugins lease the Rust frame allocation directly.

Initialize once with the application support directory, subscribe before issuing
requests, and keep passwords only for the duration of a request. Dart strings
cannot guarantee immediate memory erasure; the UI clears its controllers and
never persists credentials, while Rust consumes password bytes in `Zeroizing`.

The VM-compatible native smoke command is
`dart run tool/bridge_smoke.dart <native-library> <new-data-directory>`.
It requires a built library and a fresh isolated data directory, reads the actual
host capability status, and saves/reads/forgets one device in that store. It
does not connect, activate hosting, capture, inject input, or set passwords.
