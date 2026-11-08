# SDD ledger — plan: docs/superpowers/plans/2026-11-08-application-foundation.md

Baseline769bfdd; dedicated branch feat/application-foundation. User explicitly authorized all locally possible product implementation during Hyper-V setup, hosted builds and free/open signing. Earlier P0-only development ordering is superseded; production qualification remains unchanged.

Ruling: use GitHub keyless provenance plus OS-appropriate development signatures — avoids committing signing secrets and supplies verifiable build origin; cost: these signatures do not create Apple Developer ID/Windows trusted-publisher recognition. Mac hardware proof remains pending.

Ruling: implement inline in the existing dedicated project checkout, with one independent final review — keeps the user's source path and existing diagnostics; cost: per-task implementation is author-reviewed until the whole-branch review.

Pre-flight interfaces: protocol supplies envelopes to transport; authentication supplies a private proof to core; core supplies same-user/epoch policy to clipboard/transfers; FFI wraps these engine contracts. No component may translate a worker-supplied boolean into an authenticated grant.

The detailed approved plan remains the product scope. This implementation plan decomposes the next executable foundation; native media fidelity and privileged installed lifecycle still require their separate proofs.

Task1/2 ordering ruling: implemented the private authentication proof before broker authorization tests — prevents introducing a temporary worker-trusted authentication boolean; cost: these two tasks share the initial implementation commit.

Task1: bounded Prost schema/framing and server-hello negotiation implemented. Two framing fixtures and incompatible-hello fixture observed RED then GREEN. Real broker tests observed four RED then GREEN: connection-bound single-controller grants, session/layout invalidation, lock/file/clipboard policy, held-input release and invalid pointer coordinates. This is policy logic; it does not yet inject native input.

Task2: OPAQUE/Ristretto255/TripleDH/SHA512 with Argon2id19MiB/two passes/one lane implemented. Four real cryptographic tests observed RED then GREEN, covering wrong passwords, exporter/host/connection substitution, replay, malformed data and credential serialization. Windows current-user DPAPI protection observed RED (Unsupported) then GREEN including ciphertext tampering. Machine-service secret storage and non-Windows protected secret adapters remain unfinished; no plaintext persistence fallback was added.

Verification: Windows whole Rust workspace41tests passed (4auth+1DPAPI+4broker+4protocol+28existing); see task1-tests.log. Native qualification unchanged. Flutter3.47.9 SDK archive verified against official SHA256d8e2b5977e09082c6440f89dc1aa887b94c8fbcc3b39222797046295a3f68a9e and initialized in workspace scratch.

Task3 transport: four loopback tests observed RED then GREEN over actual sockets/TLS/QUIC: matching exporter-bound authentication, UDP-unavailable TCP fallback, wrong-password/pre-auth command rejection, pinned identity-change denial. TLS1.3 only, no early data, signed handshake verification even on first contact,8KiB unauthenticated envelopes and2MiB authenticated envelopes. This is not yet a privileged broker/worker installation or complete media channel multiplexer.

Task4 data: SQLite reopen and future-schema/identity replacement tests observed RED then GREEN. Clipboard Unicode/PNG CRC/size/echo fixtures observed RED then GREEN. Three Windows transfer tests observed RED then GREEN for1MiB chunks, whole/chunk BLAKE3, pause/resume same-user epoch, empty/cancelled/collision/approved overwrite and invalid names. Capability-directory publication never traverses a peer path; overwrite approval is a trusted local-agent argument, not a wire field. Partial data currently stays in an anonymous private tempfile while the receiver lives; process-restart journals and native clipboard integration remain pending.

Ruling: use MIT for independently written DeskHarbor source and publish the development source/CI before product completion — the user requested open source and hosted builds while local prerequisites are prepared; cost: repository contents must clearly remain marked under development. No production release or certification is implied.

Verification: Windows53tests passed after the transport/data additions; Windows strict whole-workspace Clippy passed. WSL whole-workspace tests completed successfully; detailed counts and Unix-only transfer case retained in linux-tests.log. libc was pinned forward from0.2.177 to0.2.191 to satisfy current Tokio/mio; native source semantics were unchanged and WSL regressions ran.
