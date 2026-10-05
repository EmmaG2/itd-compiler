# Combined syntax and semantic diagnostics — ODD task

## Objective and authorization

The user authorized a small compiler change so the Analyze view can show an independent syntax error and semantic error from one source. Current `analyze_internal` gates semantic analysis on an empty lexer/parser diagnostic list. The parser retains recovered AST, but a malformed return may be dropped and produce a false E2009 missing-return error if semantics is simply ungated.

## Scope and constraints

- Run semantics on recovered AST when lexer/parser diagnostics exist, retaining all original diagnostics.
- Suppress the known missing-return cascade for a function/method containing a syntax error, without suppressing independent semantic errors elsewhere. Treat recovered semantics/trace as partial; never compile or execute invalid source.
- Verify the Product/Cart reproduction: invalid decimal assignment in Product plus incomplete `return ... + ;` in Cart must report both categories, not a synthetic missing return. Check diagnostic locations and native Analyze DTO.
- Preserve valid-source behavior, compiler/project APIs, and all existing unrelated user changes. No remote operations.
- Route: delegated direct writer, because core, tests, native boundary, and documentation span multiple non-trivial files. TDD ON from global `strict_tdd: true`; `nix-shell --run 'make test-core'` then `make test` for native and `make check` for closure. Browser smoke has no Make runner.
- Delivery strategy `ask-on-risk`; forecast ~150–300 authored changed lines. Work-unit commits on this feature branch. RDD is configured ON but CLI was unavailable in previous turns; recheck and do not infer review authority.

## Task

- [ ] **CD-01 — Recover independent semantic diagnostics.** RED for the exact two-error source; GREEN with scoped semantic analysis of recovered AST and cascade suppression; native DTO proof; update documentation. Record checks, commit, review outcome and rollback.

## Evidence and next step

- Base: `feat/remove-desktop-chrome` local HEAD. Parser block recovery retains Product and Cart, but drops the invalid return; semantic method check would emit E2009. `compile_analysis` already withholds bytecode whenever any diagnostic has error severity. `analysis_dto` already maps combined diagnostics.
- Next: delegate implementation, review diff, run full checks, commit work unit, mirror this document in Engram.
