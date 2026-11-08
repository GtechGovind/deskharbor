# SDD ledger — plan: docs/superpowers/plans/2026-11-08-capture-composition.md

Baseline:8938429. User reiterated full implementation, public open-source repository, local signed artifacts and release publication once complete. User confirmed they can enable Hyper-V/restart and have no Mac/DeveloperID currently available.

Ruling: continue missing P0 capture implementation while VM setup is pending — keeps the approved milestone order and makes useful native progress without pretending that software fixtures are hardware proof; cost is that this is still not a usable remote application.

Ruling: work in the existing dedicated project checkout on new feature branch feat/capture-composition — preserves the user's exact source location and audited predecessor; no unrelated worktree or main branch changes.

Preflight interfaces: dh-media is independent; dh-platform consumes its frame/composition types; Linux C emits the documented packet. Parent owns all files inline to prevent interface drift. Existing DHF1 mode query and Windows API remain unchanged.

Task1: pending. Task2: pending. Task3: pending. Task4: Hyper-V instruction provided; remaining status pending.
