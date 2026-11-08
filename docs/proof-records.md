# Operator proof records

The gate validates **human attestations plus artifact integrity**. It cannot establish that an operator actually performed a physical action, that a video is authentic, or that an OS permission/signature was verified. Independent lab review is mandatory. A ready P0 checklist only permits the next development milestone; it does not qualify the product for release.

Start with `dh-platform-probe gate-template`. All24 records are `not_run`. Use the exact profile and case IDs from that template. `build_id` must identify the binaries being tested; record source revision and binary hashes in the evidence itself.

For each genuinely passed case, supply `environment: native_lab`, detailed observations, artifact list and a structured `attestation`:

```json
{
  "operator": "identified lab operator/reviewer",
  "observed_utc": "2026-11-08T07:00:00Z",
  "build_id": "same exact build ID as evidence root",
  "checks": [{
    "check": "capture",
    "expected": "independently visible OS sign-in reference target",
    "observed": "describe actual pixel comparison and failures",
    "artifact": "windows/boot-capture-and-observer.webm",
    "locator": "00:12-00:18 and observer log line42"
  }],
  "macos": null
}
```

Each case requires its own named checks; their exact list is defined by `required_checks` in `crates/dh-platform/src/gate.rs`. Examples: Windows sign-in needs capture/input/boot_sign_in; Linux complete_planes needs reference_comparison/cursor_overlays/direct_scanout/rotation/mixed_monitors/modifiers; Android consent needs consent_accept/consent_deny/fresh_consent. Each expected/observed check must reference a nonempty artifact of `kind: observation`, with a specific locator. Generic `diagnostic` reports cannot qualify alone. A long video may cover multiple named checks only when the relevant segments and observed results are individually identified.

Artifacts contain `path`, SHA-256 and `kind` (diagnostic/observation/signing/permissions). Stage regular, nonempty files no larger than1GiB each under a private, immutable evidence directory. Split larger recordings into bounded segments. Paths must be portable relative names without colon, backslash or traversal. Symlinks/devices/FIFOs are refused. Do not validate an attacker-writable evidence tree: path checking is not an anchored filesystem sandbox and cannot prevent malicious changes to intermediate directory components. Keep the tree unchanged throughout validation.

macOS additionally needs `attestation.macos`: `roles` for hostd/agent/capture-helper with exact installed `/Library/DeskHarbor/P0/dh-ROLE` paths, `org.deskharbor.p0.ROLE` identifiers, one common ten-character team_id and recorded designated_requirement; `launch_domain`; `security_session` (loginwindow for boot/logout, aqua_second_user for second-user, aqua/loginwindow for permission revocation); `permission_state` (granted or revoked_and_denied); and `signing_artifact`/`permission_artifact` paths of their corresponding kinds. Retain actual signature verification, permission settings and bootstrap/session evidence. The ordinary macOS inspection output is diagnostic evidence only.

None of the unit-test fixture attestations in source is a real lab result. The checked-in proof-evidence and gate-result files intentionally preserve blocked status.
