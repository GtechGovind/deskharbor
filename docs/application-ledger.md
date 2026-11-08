# SDD ledger — plan: docs/superpowers/plans/2026-11-08-application-foundation.md

Baseline769bfdd; dedicated branch feat/application-foundation. User explicitly authorized all locally possible product implementation during Hyper-V setup, hosted builds and free/open signing. Earlier P0-only development ordering is superseded; production qualification remains unchanged.

Ruling: use GitHub keyless provenance plus OS-appropriate development signatures — avoids committing signing secrets and supplies verifiable build origin; cost: these signatures do not create Apple Developer ID/Windows trusted-publisher recognition. Mac hardware proof remains pending.

Ruling: implement inline in the existing dedicated project checkout, with one independent final review — keeps the user's source path and existing diagnostics; cost: per-task implementation is author-reviewed until the whole-branch review.

Pre-flight interfaces: protocol supplies envelopes to transport; authentication supplies a private proof to core; core supplies same-user/epoch policy to clipboard/transfers; FFI wraps these engine contracts. No component may translate a worker-supplied boolean into an authenticated grant.

The detailed approved plan remains the product scope. This implementation plan decomposes the next executable foundation; native media fidelity and privileged installed lifecycle still require their separate proofs.
