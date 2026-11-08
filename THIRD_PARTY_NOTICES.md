# Third-party notices — P0 prototype

No TightVNC, VNC/RDP server, libvpx or libdrmtap source is incorporated in this prototype. Platform probes call operating-system APIs. The selected future Flutter/codec/transport dependencies are not yet part of this gated build.

The Rust dependency graph and exact versions are recorded in Cargo.lock. Cargo metadata reports MIT/Apache-2.0 alternatives for most crates, MIT for generic-array, Unlicense/MIT for memchr, Apache-2.0/BSL-1.0 for ryu, and an additional Unicode-3.0 notice for unicode-ident. Preserve the corresponding package license files when redistributing binaries. Dependencies include serde, serde_json, sha2, libc, windows-sys and their transitive build/runtime dependencies.

The Android harness uses Kotlin standard library (Apache-2.0); Android SDK/framework components remain governed by their upstream terms. Gradle8.9, AGP8.7.3, Kotlin2.0.0 and JUnit4.13.2 are build/test dependencies. The generated Gradle wrapper retains its upstream header/license.

This file inventories dependencies; it does not select a license for the newly written DeskHarbor code. Release packaging, complete license texts, signing/notarization and distribution review remain part of Milestone5.
