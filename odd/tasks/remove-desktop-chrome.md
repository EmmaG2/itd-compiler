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

- [x] **DC-01 — Remove header and project-only controls from the desktop UI.** Removed dead App-only state/handlers and updated smoke expectations, retaining backend project APIs. Verified source actions and no project controls in desktop/mobile layout. Committed a focused Conventional Commit work unit.

## Evidence and next step

- Baseline screenshots: header “ITD Compiler / Listo” and “Abrir proyecto / Analizar proyecto / Ejecutar proyecto”. Existing App.svelte contains the blocks and project-only callbacks; native/IPC APIs are separate.
- Commit `d717f74aed02adad3e5c960ba165217c31b07ade` from planning base `397740a`: 39 additions + 135 deletions in App, browser smoke, and README. Rollback boundary: revert this work-unit commit; compiler and IPC project APIs were not changed.
- RED: the user screenshots showed the unwanted header and project controls. GREEN: Playwright against `make web` at 1200 and 390 px observed no header/project buttons, all three source action buttons present, and no horizontal overflow. The browser smoke assertions were updated but the exported smoke function was not executed because no Make target/standalone runner exists.
- Focused `nix-shell --run 'make test-ui compile format-check'` passed: 17 UI tests; Svelte 0 errors/0 warnings; frontend build succeeded with an existing >500 kB chunk warning. Parent `nix-shell --run 'make check'` passed, including Rust and frontend checks; final post-edit `make format-check` and `git diff --check` passed. Full check log: `/tmp/remove-desktop-chrome-check.log`.
- RDD CLI remains unavailable in PATH and inside Nix; assessment/preflight unavailable, candidate due/unreviewed with no inferred tier or approval. The 29 pre-existing dirty tracked paths have the same diff hash as before this task; no unrelated changes were staged. No remote operation or PR was performed.
- Next: no local work remains for this UI removal. Semantic-stack SS-06 manual native proof remains a separate pending acceptance check.
