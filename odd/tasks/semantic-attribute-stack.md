# Semantic attribute stack — ODD implementation plan

## Objective, problem, and rationale

Implement a real semantic attribute stack in the Rust compiler and an inspectable
trace in the desktop application. Expression reductions must consume attributes
from the stack to validate operands and produce their result types; a decorative
recording of independently calculated types does not satisfy this objective.

The existing scope stack controls symbol visibility. The VM operand stack
controls execution. Neither substitutes for this semantic attribute stack.

## Authorization and current status

- User authorized implementation of this plan and subsequently confirmed
  `feature-branch-chain` for delivery.
- Authorized: scoped local implementation, feature branches, tests, documentation,
  ODD work-unit commits, and the full Engram recovery mirror.
- Current step: implement all six tasks under the user's explicit request to
  complete the feature in one run, preserving existing working-tree changes.
- Push, PR creation, merge, and remote operations remain unauthorized. Native
  reviews remain subject to the effective mode and candidate-specific consent.
- Status: SS-01 through SS-05 implemented, functionally verified, and committed;
  native WebView inspection remains pending. SS-06 documentation is next.
- Repository locator: `odd/tasks/semantic-attribute-stack.md`.
- Engram topic: `odd/semantic-attribute-stack/tasks`; project: `itd-compiler`.
- No SDD artifacts or synthetic SDD run.
- No code changes or functional test results are claimed by this plan.

## Baseline and verified evidence

- Planning date: 2026-10-04.
- Current branch: `feat/dual-tree-visualization`.
- Verified dependency baseline: `ebc0ef284b916b0575b35e8d843656d42d959c6d`.
  Local `main` is at `32fd5a0` and lacks this dependency; remote state is unverified.
- Preserve the existing dual-tree task document and all unrelated working-tree
  changes: `.prettierrc`, 28 frontend files, `.atl/`, and local `.codegraph/`.
- Never stage unrelated changes or infer clean-commit verification from a dirty
  worktree. Use the dependency baseline and delivery branches defined below.
- Read-only mapping used CodeGraph and actual project configuration.
- `semantic.rs:667`: expression analysis; `:776`: type recording;
  `:861`: assignment; `:920`: binary rules.
- `semantic.rs:752`: qualified module names can resolve without evaluating a
  module identifier as an ordinary value.
- Assignment analyzes the RHS first. A name destination resolves directly rather
  than being read, preserving initialization checks.
- `project.rs:223` uses `analyze_with_imports`; ordinary and imported analysis
  must share the stack implementation.
- `parser.rs:439` uses precedence/binding power, not LR reductions.
- Native inspection is coordinated in `apps/desktop/src-tauri/src/lib.rs`;
  TypeScript contracts live in `apps/desktop/src/lib/types/compiler.ts`.

## Academic boundary and non-goals

This plan implements bottom-up semantic attribute reductions over the AST.
The user authorized this AST-based implementation plan. The professor's exact
rubric remains unavailable: do not claim this is an LR parser semantic-action
stack or guarantee rubric compliance. If a later rubric requires parsing-time
reductions, revise the affected tasks before expanding implementation scope.

Do not replace the parser, invent a second type system, change language rules,
enable currently unsupported indexation, alter VM execution, replace either tree,
or add dependencies without a concrete need. Multi-module analysis must use the
stack, but multi-module trace display and changes to ProjectDto are out of scope.

## Technical approach and invariants

Use a small internal `semantic_stack.rs` module. Attributes contain `ExprId`,
`Type`, and `Span`; IDs link to existing resolutions and coercion tables.

Separate stack-producing expression evaluation from the existing Type-returning
adapter used by declarations, conditions, and returns. Reductions consume their
children's attributes and push one result. Existing type tables record that
result; do not keep an independent type-calculation path.

1. Evaluating a node at height h ends at h + 1, including ordinary semantic errors.
2. A reduction consumes only its own child segment and preserves enclosing frames.
3. Binary operands are visited left-to-right and popped right-before-left.
4. Calls preserve callee/argument order, arity, constructor and visibility rules.
5. Consuming a statement's final expression restores its initial height.
6. Analysis ends with no residual attributes.
7. Checkpoints isolate recovery to the current expression; use Type::Error without
   changing existing user diagnostics or contaminating following expressions.
8. Unexpected IDs or underflow are internal invariant failures, handled explicitly
   through controlled diagnostics rather than unchecked panics or silent success.
9. Preserve RHS-first assignment, non-reading lvalues, qualified-module lookup,
   imported symbols, and ExprId-keyed implicit conversions.
10. Plain and traced analysis produce equivalent semantic results and bytecode.

Add an opt-in trace API while preserving existing public entry points and
SemanticResult consumers. Trace push/reduce/consume, relevant conversion, and
recovery operations as they happen. Define one coherent delta/event contract
whose replay reconstructs the stack, including the first and final states.

Bound capture inside the core by event count and bounded payloads. Avoid full
stack snapshots per event. Expose truncation explicitly; stop recording rather
than truncating actual semantic evaluation. Enforce caps before IPC serialization.
Existing native source/token/symbol/graph limits remain unchanged.

## TDD, checks, and review configuration

- TDD: ON, observed `strict_tdd: true` in `~/.gentle-ai/state.json`.
- Clone-local `.gentle-ai/state.json` was absent at planning time.
- Recheck effective configuration before implementation/resume; record any change.
- Each behavioral task requires observed RED, GREEN, then REFACTOR evidence.
- Core runner: `nix-shell --run 'make test-core'` (includes reference interpreter).
- UI runner: `nix-shell --run 'make test-ui'` (Vitest).
- Native DTO tests require `nix-shell --run 'make test'`; test-core excludes Tauri.
- Build/type checks: `nix-shell --run 'make compile'`.
- Closure: `nix-shell --run 'make check'`.
- RDD: global configuration says ON; native effective status is unverified because
  `gentle-ai` was unavailable in the current PATH. Do not infer low risk or approval.
- No review is started by this delivery-definition step. At implementation,
  resolve native mode read-only and follow candidate consent and exact native
  transitions. Record each commit's assessed tier/outcome or unavailable status.
- Browser smoke exports a function receiving a Playwright page; there is no Make
  target or standalone runner. Do not bypass Make or invent a passing command.
  Interactive/native checks must be reported separately from mock IPC evidence.

## Delivery forecast and commit policy

- Strategy: `ask-on-risk`; chain strategy: `feature-branch-chain`, explicitly
  confirmed by the user for this feature. Do not ask for the chain choice again.
- Implementation forecast: 1,290–2,090 authored additions plus deletions, generated
  files excluded. Add actual planning-document overhead to the delivery total.
- Running committed count: 0. Proposed slices are listed below; actual commit
  boundaries, PR numbers, measured counts, and review receipts remain pending.
- The 400-line task size is advisory, not an acceptance gate or reason to omit tests,
  minify, or split a coherent expression migration artificially.
- Future implementation tasks close with Conventional Commit work units containing
  behavior, tests, and relevant docs. No AI attribution or Co-Authored-By.
- Push, PR creation, merge, and remote execution require separate authorization.
- Read work-unit-commits and chained-pr by registry name before PR planning.
- The first review boundary is the selected branch point; later boundaries follow
  native acknowledgements. Checkboxes are not review receipts.

### Branch and dependency chain

Create the local tracker from the exact verified dependency commit, not from dirty
working-tree contents. The dual-tree dependency is an explicit exception to the
usual tracker-from-main starting point; it avoids mixing the existing feature
into semantic-stack review slices.

```text
feat/dual-tree-visualization @ ebc0ef2  (prerequisite)
  feat/semantic-attribute-stack       (tracker)
    feat/semantic-attribute-stack-01-core
      feat/semantic-attribute-stack-02-analysis
        feat/semantic-attribute-stack-03-trace
          feat/semantic-attribute-stack-04-ipc
            feat/semantic-attribute-stack-05-ui
              feat/semantic-attribute-stack-06-docs
```

Each indentation level is based on its immediate parent. These branches are
planned, not created. Keep the planning-document work unit on the tracker and
include its actual size as overhead. Each child contains its task's behavior,
tests, relevant documentation, and progress evidence.

1. **01-core / SS-01:** checked stack operations; base: tracker.
2. **02-analysis / SS-02:** full evaluator integration; base: 01-core.
3. **03-trace / SS-03:** optional bounded trace; base: 02-analysis.
4. **04-ipc / SS-04:** native/TypeScript inspection contract; base: 03-trace.
5. **05-ui / SS-05:** teaching panel and lifecycle; base: 04-ipc.
6. **06-docs / SS-06:** educational examples and verified closure; base: 05-ui.

When remote delivery is separately authorized, all PRs start as drafts. Child 1
targets the tracker; each later child targets the previous child. A tracker opened
before the prerequisite lands must target the verified dependency branch and stay
draft/no-merge, not target the current local main baseline. After the prerequisite
lands, verify actual ancestry and retarget/rebase as needed before targeting main.
Do not infer remote merge status or PR identities from local branch names.

Integrate children in dependency order only with merge authorization. Preserve
focused downstream diffs as bases advance. The tracker remains non-mergeable until
the prerequisite, all tasks, applicable reviews, and final checks are complete.

### Slice limits and verification

Measure every child against its immediate parent, not main or the accumulated
feature branch. Record start/end commits, additions plus deletions, and changed
code-file count before delivery. Target at most 400 changed lines per child PR.
Keep repository hard limits of fewer than 1,000 changed lines and fewer than 10
code files; a 400-line exception does not waive those limits.

SS-02's 350–550-line estimate is an explicit size risk, not an approved exception.
If actual size exceeds the review budget, make one honest responsibility-based
slicing attempt. If no coherent split exists, retain correct behavior and tests,
report the measured range and rationale, and request `size:exception` before that
delivery. Do not create a partial type system, separate tests from behavior, omit
docs, minify, or delete useful whitespace to fit. Apply the same rule to any other
slice that exceeds its forecast; six proposed slices are not six guaranteed sizes.

For each candidate, run its task-specific Make checks and the applicable native
review flow. Before publishing any slice, run `make check` through Nix and report
any failure. Final acceptance also requires the native demonstration; mocked IPC
alone is insufficient. A passing dirty-worktree check is not clean-commit proof.
Every future PR records its immediate base, dependency diagram, scope, rollback,
test evidence, and pending work without replacing the repository PR template.

## Tasks

All tasks are authorized for local implementation but remain pending.
Each task uses delegated direct implementation; the parent owns scope, checks,
document/mirror updates, and delivery decisions.

- [x] **SS-01 — Internal attribute stack and checked operations**
  - Depends on: none. Estimate: 200–320 authored lines.
  - Files: new core `src/semantic_stack.rs`, module registration in `src/lib.rs`,
    in-module tests.
  - Route: delegated; preparation and multiple non-trivial file changes.
  - Acceptance: typed attributes, segment consumption, checkpoints, LIFO order,
    isolation, and controlled underflow handling.
  - Proof: RED/GREEN core tests; `make test-core`.
  - Rollback: remove the unused stack module, registration, and associated tests.

- [x] **SS-02 — Stack-driven expression analysis**
  - Depends on: SS-01. Estimate: 350–550 authored lines.
  - Files: `semantic.rs`, dedicated `tests/semantic_stack.rs`, relevant semantic tests.
  - Route: delegated; multiple non-trivial files and complete evaluator migration.
  - Acceptance: all expression variants use stack-fed rules; balanced nesting;
    preserve diagnostics, coercions, symbols, imports, assignments, and module names.
    Unsupported indexation remains unsupported.
  - Proof: RED/GREEN regressions for subtraction/division, precedence, calls,
    lvalues, members, casts, error recovery and imported symbols; `make test-core`.
  - Rollback: restore the former evaluator and its tests; retain SS-01 independently.

- [x] **SS-03 — Bounded optional semantic trace**
  - Depends on: SS-02. Estimate: 220–360 authored lines.
  - Files: stack module, `semantic.rs`, `lib.rs`, core trace tests.
  - Route: delegated; cross-file API and instrumentation work.
  - Acceptance: events come from real stack operations; opt-in entry points preserve
    existing APIs; coherent replay, conversions/recovery, bounded capture,
    explicit truncation, and identical plain/traced results.
  - Proof: RED/GREEN equivalence, boundary-limit, nesting, recovery and import tests;
    `make test-core`.
  - Rollback: remove trace APIs and capture while retaining functional stack analysis.

- [x] **SS-04 — Native inspection and IPC contract**
  - Depends on: SS-03. Estimate: 180–300 authored lines.
  - Files: native `lib.rs` and optional inspection module; `types/compiler.ts`;
    actual IPC test fixtures and `tests/browser-smoke.mjs` fixture.
  - Route: delegated; Rust/TypeScript contract synchronization.
  - Acceptance: bounded trace DTO, source locations, truncation, null/empty states;
    no ProjectDto expansion, permission changes, or weakened source limits.
  - Proof: RED/GREEN native DTO and frontend contract tests; `make test`,
    `make compile`. Record mock-vs-native boundaries explicitly.
  - Rollback: remove trace DTO exposure and matching fixtures, preserving core APIs.

- [x] **SS-05 — Semantic stack teaching panel**
  - Depends on: SS-04. Estimate: 250–400 authored lines.
  - Files: new `features/semantic-stack/` view and replay utility/tests;
    `App.svelte`, `locales/es.json`, relevant smoke assertions.
  - Route: delegated; multiple non-trivial UI/lifecycle files.
  - Acceptance: step controls, current stack, rule/operands/result, source navigation,
    accessible empty/error/truncated states, and clear-on-edit/project invalidation.
    Keep both existing trees; no third tree. UI text follows existing Spanish locale.
  - Proof: RED/GREEN replay and lifecycle tests; `make test-ui`, `make compile`;
    interactive desktop demonstration, honestly pending if unavailable.
  - Rollback: remove the panel and trace presentation while retaining IPC/core trace.

- [ ] **SS-06 — Educational documentation and verified closure**
  - Depends on: SS-05. Estimate: 90–160 authored lines.
  - Files: `docs/language/semantic-stack.md`, language index, README,
    browser verification README; examples/catalog only if new fixtures are needed.
  - Route: delegated; multiple documentation/test-fixture files and verification.
  - Acceptance: explain attributes and AST reductions, distinguish the three stacks,
    document limits, demonstrate `2 + 3 * 4`, conversion, call, and error recovery.
    Reuse the shared examples catalog rather than copying examples into the UI.
  - Proof: all applicable regressions; `make check`; manual native demonstration.
    Report failed/skipped/unavailable checks and clean-commit verification limitations.
  - Rollback: remove feature documentation/examples without changing analysis behavior.

## Evidence ledger and recovery

### SS-05 implementation evidence

- Base: `cde2dfc`; source commit `c8a34204355fa949dd8e89c120e270dd6e99f378`:
  486 additions + 24 deletions, six files. New component and pure replay utility
  reuse existing editor navigation, result tabs, buttons and Spanish locale.
- RED: Nix `make test-ui` failed on missing replay module. A later real-delta
  regression failed because a synthetic combined-pop test hid missing operands
  on actual Reduce events; fixed by accumulating preceding Consume attributes
  across zero-delta conversions, without replaying a fabricated pop.
- GREEN: `make test-ui compile format-check` exited 0 after corrections:
  17 frontend tests across six files, Svelte check 0 errors/0 warnings,
  production build succeeded with existing >500 kB chunk warning.
- Browser smoke fixture now uses a real push/push/consume/consume/reduce/consume
  sequence and asserts step state, operands/result, source navigation, reset and
  source-edit invalidation. The fixture itself has no Make runner and was not run.
- Manual Playwright inspection with mocked IPC on `make web` (localhost:5174)
  observed two operands and result at Reduce, editor focus on source navigation,
  reset to step zero on reanalysis, and trace removal on source edits.
- Visual RED: new fifth tab wrapped Bytecode onto a second row while Tabs.List
  height remained 36px versus scrollHeight 65px, overlapping panel help. Fixed
  list height while preserving existing palette. Browser geometry GREEN:
  1200px list height 66px, bottom 446 < help top 462; 390px list height 66px,
  bottom 439.9 < help top 455.9. Mobile panel screenshot captured locally.
- Parent ran scoped checks and the existing Make formatter only after writers
  paused; all 29 pre-existing tracked dirty paths retained byte-identical diffs.
- Native Tauri WebView click-through remains pending, distinct from browser mocks.
  RDD assessment/preflight unavailable (127), due/unreviewed.
- This slice is 510 authored lines before tracking overhead. A second coherent
  PR slice was not made from replay alone because it would not deliver inspection;
  do not shrink tests or UI to fit. Seek `size:exception` or a valid separate
  deliverable before any remote PR; no exception was granted.

### SS-04 implementation evidence

- Base: `15289ff`; source commit `a47e7591303710f334f420551c060fe81eae770c`:
  245 additions + 5 deletions across five code/fixture files.
- RED: Nix `make test` failed on missing native inspect_source. Two intermediate
  IPC mock failures (missing window and optional invoke argument) were fixed;
  final `make test` exited 0: all core suites, 10 native tests, 12 frontend tests.
- `make compile` passed (existing >500 kB Vite chunk warning); final
  `make format-check` passed. Native tests exercise real compiler traces, Unicode
  positions, semantic/syntax/empty states, event/type bounds and conversions.
- Refactor isolates bounded projection in native semantic_trace.rs while retaining
  analysis_dto compatibility. No dependencies, source limits, permissions, or
  ProjectDto changes. Frontend IPC tests verify transport, not native WebView UI.
- Parent ran the existing Make formatter after all writers paused, then verified
  the 29 pre-existing tracked dirty paths retained byte-identical diffs. No user
  changes were staged. Logs: `/tmp/ss04-red.log`, `/tmp/ss04-green3.log`,
  `/tmp/ss04-compile.log`; earlier failed logs are superseded, not hidden.
- RDD unavailable (assessment/preflight exit 127), due/unreviewed. Implementation
  running total: 1,150 authored lines, excluding tracking docs.

### SS-03 implementation evidence

- Base: `8a2fddd`; source commit `0b7b4ec0412656da7d01a2cdcdf86b8ee0c8aac9`:
  428 additions + 32 deletions, four code files. Core total: 900 authored lines.
- RED: Nix `make test-core` failed on missing trace APIs. GREEN: 43 core tests,
  including four new integration tests and one forced-recovery unit test.
- Verified identical plain/traced analysis and bytecode; replayable complete and
  truncated traces; zero/exact/hard limits; syntax-unavailable and empty-source
  behavior; zero-delta coercions; imported symbols and bounded type labels.
- Capture is opt-in, capped inside the core at 4096 events. Labels are generated
  lazily and limited to 80 characters without expanding recursive function types.
  Consume events remove actual operands; Reduce events push computed results.
- Refactor centralized coercion recording. `make lint` passed. Initial final check
  found one new assert formatting issue; corrected manually, then standalone
  `make format-check` passed. All runners used Nix; logs: SS03 files under `/tmp`.
- Slice is 460 authored lines before tracking overhead. One responsibility pass
  retained capture/API/tests together because an unused trace recorder or an API
  without actual capture would not deliver the required behavior. No code/tests
  were compressed. A size exception or revised coherent PR split remains required
  before remote delivery; none was granted or inferred from chain selection.
- RDD assessment/preflight remain unavailable (127); due/unreviewed. No PR exists.

### SS-02 implementation evidence

- Base: `02b7109` on the prior child; source commit
  `ed277db956a54c35b70130ab0d3e304639885535` (216 additions + 75 deletions).
- RED: `make test-core` via Nix failed on the missing evaluator constructor before
  implementation. GREEN: same command exited 0, 38 tests including five new tests.
  Internal tests directly verify one result attribute and no residual stack state;
  integration tests cover operand/call order, errors/coercions, lvalues and imports.
- Refactor preserves existing rules while extracting Analyzer construction and
  consuming real attributes. Type::Error behavior is retained; E2099 identifies
  internal stack invariant failures, not ordinary source errors.
- `make lint` exited 0 (Clippy and Svelte 0 errors/warnings); final
  `make format-check` exited 0 after manually correcting new Rust formatting.
  All commands ran through Nix; logs use `/tmp/semantic-stack-ss02-*.log`.
- Runtime boundary: core analyze/run and existing VM/reference regression suites.
- RDD assessment/preflight unavailable (exit 127); due/unreviewed, no approval.
- Core implementation total so far: 440 authored lines, excluding tracking docs.

### SS-01 implementation evidence

- Tracker planning commit: `eef2514`; first child based on that commit.
- Core commit: `0f06dfb8ca1a65ba7ef9f3f7a3d21c89c8993702`, 149 authored additions,
  two files. Delegated writer; parent inspected stack API and test output.
- RED: `nix-shell --run 'make test-core'` failed with missing stack types before
  implementation (Make error 101). GREEN: same command exited 0; four new stack
  tests and all existing core suites passed. Logs: `/tmp/semantic-stack-ss01-red.log`
  and `/tmp/semantic-stack-ss01-green.log` (local, non-delivery artifacts).
- Refactor: retained small checked Vec API. Module is temporarily test-only until
  SS-02 consumes it, avoiding dead code in the intermediate production build.
- `git diff --check` and staged whitespace check passed. Runtime boundary: N/A,
  private stack operations only; native/VM behavior is not changed in SS-01.
- RDD configured ON; mode lookup, assessment, and preflight attempts are unavailable
  (CLI missing, exit 127), including inside Nix. Candidate remains due/unreviewed;
  no risk tier, consent, approval, or reviewed-boundary advance is fabricated.
- Existing unrelated dirty changes are preserved. Remote delivery remains off.

Planning evidence: CodeGraph mapping, actual Makefile commands, global TDD/RDD
configuration, current branch/status, and the existing dual-tree task document.
No implementation tests, RED/GREEN runs, commits, review receipts, or demos exist.

Planning verification: `nix-shell --run 'make format-check'` exited 0 on the current
working tree; Rust format and Prettier passed, including this new document.
`git diff --check` exited 0 for existing tracked changes. The local plan and full
Engram observation #84 were read back and matched exactly before this evidence
update. No compiler/UI tests or native reviews were run for planning. Native mode
status remains unavailable in the current PATH. No branch or commit was created.

Delivery definition: user confirmed the chain strategy. Parent read the complete
task file and Engram observation #84; delegated read-only preparation checked the
dependency/base and slice boundaries. Parent updated this single document inline.
No implementation task, branch, commit, remote PR, or merge is complete.

For each task, append observed RED/GREEN/REFACTOR, command/exit/result, runtime
boundary, commit identity, authored line count, native review tier/outcome, rollback
notes, and next step. Check off only after its outcome and applicable checks pass.
Update this file and the full Engram mirror after each task, then read both back.
Keep failed/pending mirror writes explicit rather than claiming synchronization.

Resume: mem_context → project/feature mem_search → full mem_get_observation →
actual task file; reconcile authorization, configuration, source, and proof before
continuing. Preserve conflicts rather than overwriting another feature's state.

## Next step

Delivery strategy is defined and implementation is already authorized. Resume
SS-01 after rechecking TDD/native review availability and reading back this document
and its full mirror. Establish the local tracker and first child from the recorded
dependency baseline while preserving unrelated dirty changes. Record the actual
branch point, then follow task-by-task TDD and work-unit verification. No additional
chain confirmation is needed; remote delivery remains a separate user decision.
