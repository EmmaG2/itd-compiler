# Dual compiler tree visualization

## Objective and authorization

Implement the approved two-tree plan in the existing AST area: syntax AST and a separate semantic graph derived from the AST plus real semantic results. The user explicitly requested implementation. The semantic view demonstrates syntactic/semantic integration; no third view, concrete derivation tree, semantic parser stack, compiler IR rewrite, or new dependencies.

## Baseline and constraints

- Feature branch: `feat/dual-tree-visualization`; branch point: `32fd5a03ea373c5d61a76927132e9b91ba771418`.
- Preserve pre-existing `.prettierrc` changes and `.atl/`; keep local `.codegraph/` out of commits.
- Reuse existing AST builder, graph layout, UI controls, and Spanish locale. Code identifiers remain English.
- Keep `ast`; add nullable `semanticAst` with expression types, resolved symbols, and explicit implicit-conversion nodes. No frontend type inference or invented semantic results.
- Preserve source locations and the 2000-node cap per tree without orphan nodes. Clear both graphs on source changes.
- Syntax errors make semantics unavailable; semantic errors allow an explicitly partial/error view.
- TDD: ON, from `~/.gentle-ai/state.json` (`strict_tdd: true`), no discovered local override. Observe RED before implementation, GREEN, then refactor.
- Runners: `nix-shell --run 'make test'` (Rust workspace including Tauri, then frontend), `nix-shell --run 'make test-ui'` for UI iteration, and `nix-shell --run 'make check'` for final verification. All project tasks use Make.
- RDD: ON (global). Initial unrelated `.prettierrc` assessment: medium/under_budget after excluding untracked files. Feature assessments must use committed-only from the last reviewed boundary, initially the branch point. Never include unrelated changes in the feature candidate.

## Delivery

- Strategy: `ask-on-risk`; use recommended `feature-branch-chain` locally after the user instructed continuation in response to the delivery question. Keep backend and UI integrated; no push or PR authorized. Updated forecast: approximately 620 authored additions + deletions including task tracking (T1 commit 227, T2 UI 351 plus empty-source correction and verification records). Do not shrink tests or minify to meet the heuristic. Planned slices: T1 native projection/contract, then T2 UI depending on T1.
- Running authored line count: 227. Review boundary remains branch point (T1 medium/under_budget). Slice 1: `32fd5a03..c0ad67b`; slice 2 starts after `c0ad67b`. No remote PRs created.
- Route: delegated direct for both tasks; preparation and writer triggers apply (multiple non-trivial files across native IPC and UI). No SDD artifacts.

## Tasks and acceptance

- [x] **T1 — Native semantic graph and contract.** Delegated: builder/DTO, matching TypeScript, native tests. Preserve syntax graph, expose true inferred types and stable symbol identities, insert real coercions with source spans, handle absent/error semantics and truncation. Acceptance: native tests cover syntax/semantic separation, inferred types, shadowing, coercion, unavailable/partial semantics, valid parent links and limits. RED/GREEN via `make test`; focused rollback removes only the added semantic projection/contract/tests. Runtime boundary: native analysis DTO tests. Commit: `c0ad67b4f44290903a909ab18d290410b6800e28` (`feat(compiler): expose semantic tree projection`). RDD: medium / under_budget (227 lines); native review deferred, boundary unchanged.
- [ ] **T2 — Two-view UI and verified delivery.** Delegated: selector, semantic node metadata, dynamic fullscreen labels, invalidation, browser fixture, tests and concise user docs. Acceptance: exactly two tabs, syntax selected initially, shared zoom/fullscreen/source navigation, Spanish labels, honest empty/error states, no stale graph; demo `let count := 2; let total := count + 0.5dec;` demonstrates Int-to-Decimal coercion. RED/GREEN via `make test-ui` and full `make check`; manual browser/native checks must be reported separately. Browser smoke has no Make target: do not run underlying tool directly. Rollback retains T1 contract while removing UI exposure and corresponding tests/docs. Commit and RDD assessment: pending.

## Evidence and next step

T1 implemented: 184 native + 7 TypeScript added lines. RED observed via `nix-shell --run 'make test'`: six E0609 errors for missing new graph fields before implementation. GREEN: complete `make test` exited 0, all 36 Rust tests including seven native (three new) plus eight Vitest tests across five files passed. `make lint` Rust Clippy passed, frontend svelte-check pending in session 81626. Frontend startup showed slow module filesystem reads; exact cause not established. `git diff --check` passed. T1 committed as `c0ad67b`; functional checks passed. Native risk assessment initially required explicit untracked exclusion; rerun succeeded medium/under_budget. No reviewer invoked.

Baseline and post-change `nix-shell --run 'make format-check'`: Rust formatting passed, frontend formatting failed because the pre-existing `.prettierrc` plugin entry cannot resolve `prettier-plugin-svelte`. Preserve that user configuration; do not add an unrelated dependency. Final `make check` may remain blocked by this baseline issue.

T2 implemented, awaiting commit and final verification status:

- UI RED: two expected failures (missing semantic labels and insufficient vertical spacing); GREEN: `nix-shell --run 'make test-ui'` passed 10/10 after final edits.
- Empty-source acceptance correction: native test failed on a Program-only graph before a three-line early return; `nix-shell --run 'make test'` then passed all 37 Rust + 10 frontend tests. Both empty and whitespace sources now produce empty graphs.
- Browser RED/GREEN: nested tabs initially collapsed the AST panel to 103px. Fixed flex sizing and hidden graph mounting; measured 548px panel and 1440x1000 fullscreen. Verified syntax has no semantic labels, semantic types/conversion/symbol labels, zoom/fit, node-to-editor navigation, Escape/focus restoration, partial banners, truncation notice, unavailable state, source-edit clearing and project-analysis clearing. Mobile fullscreen measured 390x844. IPC was mocked; native DTOs independently tested. Screenshot: `.playwright-mcp/dual-tree-semantic.png` (local artifact, not committed).
- `nix-shell --run 'make check'` failed at the pre-existing missing Prettier plugin; Rust formatting passed. Log: `/tmp/itd-dual-tree-check.log`. No user config changed.
- Earlier `make lint` Clippy passed; its Svelte Check was cancelled after 20 minutes of slow dependency loading (exit 143). Current `make compile` remains pending in UI worker session 90427. No completed build/typecheck claimed.
- Existing browser smoke fixture and regression assertions updated; script itself not run because no Make target exists. Equivalent interactive browser checks above passed.
- `git diff --check` passed. Next: commit T2, assess committed RDD slice from branch point, collect native review consent if due; report build/typecheck limitations honestly. Memory mirror topic: `odd/dual-tree-visualization/tasks`.
