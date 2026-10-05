# Remove redundant desktop chrome — ODD task

## Objective and authorization

The user requested removal of the visible ITD Compiler/status header and project controls shown in two screenshots. Remove the project entry-module field and its three buttons with the project-only UI path. Keep source analyze/run/stop, examples, result panels, and compiler/native project APIs intact. Do not alter unrelated user changes or perform remote operations.

## Route and constraints

- Route: delegated direct; App UI and browser smoke are two non-trivial files, with preparation for a write.
- Branch: create `feat/remove-desktop-chrome` from the current local semantic-stack child, without touching the pending native acceptance status of that feature.
- TDD: ON from global `strict_tdd: true`; use a rendered UI assertion as RED/GREEN evidence if the existing browser harness permits. The browser smoke has no standalone Make runner; do not invent a passing run. Functional checks: `nix-shell --run 'make test-ui compile format-check'`, manual visual check if browser tooling is available, and `nix-shell --run 'make check'` before closure.
- RDD: configured on, but `gentle-ai` CLI was unavailable (127) in the previous feature. Recheck; no risk tier or review receipt is inferred.
- Estimated ~100–180 authored changed lines; delivery strategy `ask-on-risk`, no chain needed unless actual work exceeds ~400 lines.

## Task

- [ ] **DC-01 — Remove header and project-only controls from the desktop UI.** Remove dead App-only state/handlers and update smoke expectations, retaining backend project APIs. Verify source workflow and no project controls in desktop/mobile layout. Commit a focused Conventional Commit work unit. Record checks, review availability, rollback, and next step here.

## Evidence and next step

- Baseline screenshots: header “ITD Compiler / Listo” and “Abrir proyecto / Analizar proyecto / Ejecutar proyecto”. Existing App.svelte contains the blocks and project-only callbacks; native/IPC APIs are separate.
- Next: branch, delegate the scoped edit, verify and commit. No changes to semantic-stack SS-06 acceptance are implied.
