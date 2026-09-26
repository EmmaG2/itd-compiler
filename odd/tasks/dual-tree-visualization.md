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

- Strategy: `ask-on-risk`; use recommended `feature-branch-chain` locally after the user instructed continuation in response to the delivery question. Keep backend and UI integrated; no push or PR authorized. Updated forecast: approximately 450 authored additions + deletions including task tracking (T1 actual 191, T2 forecast 190–250, plus documentation). Do not shrink tests or minify to meet the heuristic. Planned slices: T1 native projection/contract, then T2 UI depending on T1.
- Running authored line count: 0. Review boundary: branch point. Slice boundaries: pending.
- Route: delegated direct for both tasks; preparation and writer triggers apply (multiple non-trivial files across native IPC and UI). No SDD artifacts.

## Tasks and acceptance

- [ ] **T1 — Native semantic graph and contract.** Delegated: builder/DTO, matching TypeScript, native tests. Preserve syntax graph, expose true inferred types and stable symbol identities, insert real coercions with source spans, handle absent/error semantics and truncation. Acceptance: native tests cover syntax/semantic separation, inferred types, shadowing, coercion, unavailable/partial semantics, valid parent links and limits. RED/GREEN via `make test`; focused rollback removes only the added semantic projection/contract/tests. Runtime boundary: native analysis DTO tests. Commit and RDD assessment: pending.
- [ ] **T2 — Two-view UI and verified delivery.** Delegated: selector, semantic node metadata, dynamic fullscreen labels, invalidation, browser fixture, tests and concise user docs. Acceptance: exactly two tabs, syntax selected initially, shared zoom/fullscreen/source navigation, Spanish labels, honest empty/error states, no stale graph; demo `let count := 2; let total := count + 0.5dec;` demonstrates Int-to-Decimal coercion. RED/GREEN via `make test-ui` and full `make check`; manual browser/native checks must be reported separately. Browser smoke has no Make target: do not run underlying tool directly. Rollback retains T1 contract while removing UI exposure and corresponding tests/docs. Commit and RDD assessment: pending.

## Evidence and next step

T1 implemented: 184 native + 7 TypeScript added lines. RED observed via `nix-shell --run 'make test'`: six E0609 errors for missing new graph fields before implementation. GREEN: complete `make test` exited 0, all 36 Rust tests including seven native (three new) plus eight Vitest tests across five files passed. `make lint` Rust Clippy passed, frontend svelte-check pending in session 81626. Frontend startup showed slow module filesystem reads; exact cause not established. `git diff --check` passed. T1 commit being prepared; checkbox remains open until commit evidence is recorded.

Baseline and post-change `nix-shell --run 'make format-check'`: Rust formatting passed, frontend formatting failed because the pre-existing `.prettierrc` plugin entry cannot resolve `prettier-plugin-svelte`. Preserve that user configuration; do not add an unrelated dependency. Final `make check` may remain blocked by this baseline issue.

T2 preparation complete (read-only), awaiting T1 commit. Next: record T1 commit/assessment and authorize T2 implementation. Memory mirror topic: `odd/dual-tree-visualization/tasks`.
