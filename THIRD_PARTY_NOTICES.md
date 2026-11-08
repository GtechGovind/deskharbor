# Third-party notices

No TightVNC, VNC/RDP server, libvpx or libdrmtap source is incorporated. Platform probes call operating-system APIs. The application foundation now adds Prost, Tokio, Quinn/rustls/ring, opaque-ke/Argon2, SQLite, BLAKE3, PNG and capability-based filesystem dependencies. Exact direct/transitive versions and package checksums are in Cargo.lock.

The Rust dependency graph and exact versions are recorded in Cargo.lock. Cargo metadata reports MIT/Apache-2.0 alternatives for most crates, MIT for generic-array, Unlicense/MIT for memchr, Apache-2.0/BSL-1.0 for ryu, and an additional Unicode-3.0 notice for unicode-ident. Preserve the corresponding package license files when redistributing binaries. Dependencies include serde, serde_json, sha2, libc, windows-sys and their transitive build/runtime dependencies.

The Android harness uses Kotlin standard library (Apache-2.0); Android SDK/framework components remain governed by their upstream terms. Gradle8.9, AGP8.7.3, Kotlin2.0.0 and JUnit4.13.2 are build/test dependencies. The generated Gradle wrapper retains its upstream header/license.

Newly written DeskHarbor code uses the MIT license in LICENSE. Upstream dependencies keep their own licenses; Cargo metadata and their distributed license files are authoritative. Complete upstream license text bundling, signing/notarization and distribution qualification remain release-packaging requirements. Do not infer that all dependencies share DeskHarbor's license.
