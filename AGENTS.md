# Coding

* Follow YAGNI principles.

## Git workflow
- Never use `git rebase`. Always use `git merge` / `git pull --no-rebase`.
- Commit as `Rui Carmo <rui.carmo@gmail.com>` unless explicitly told otherwise.
- Configure both local and global Git identity before committing:
  - `git config user.name "Rui Carmo"`
  - `git config user.email "rui.carmo@gmail.com"`
  - `git config --global user.name "Rui Carmo"`
  - `git config --global user.email "rui.carmo@gmail.com"`

## Upstream release audits

`RELEASE.md` is mandatory release-audit evidence for this repository.

For every future `@earendil-works/pi-ai` upstream release audit:

1. Use the latest official published upstream tag/npm artifact and the exact prior accepted upstream tag as the only audit bounds. Do not chase upstream `main` or untagged commits unless explicitly instructed.
2. Record the previous accepted upstream tag/SHA, new upstream tag/SHA, audited range, npm package version, and npm tarball SHA-256.
3. Before implementation, mechanically derive and record:
   - the exact changed `packages/ai` path set and disposition matrix;
   - the complete upstream `packages/ai/test/*.test.ts` corpus/crosswalk;
   - full-record text catalog delta (old→new, added/removed/changed records);
   - full-record image catalog delta (old→new, added/removed/changed records).
4. Port behavior through production Rust paths, not test-local shims. Add deterministic production-path tests for parser/stream/replay/payload behavior touched by the upstream release.
5. Regenerate text and image catalogs cleanly from the official artifact. Verify full-record metadata equality, provider/id comparator equality, and deliberate text + image metadata fault gates.
6. Update `RELEASE.md`, parity ledgers/crosswalks, validators, and all audit evidence in the same candidate series as the release port/audit. Record all Rust implementation/fix/adaptation details and all N/A decisions.
7. Run and record local gates before pushing:
   - `cargo fmt -- --check`
   - `cargo build`
   - focused production-path tests for changed behavior
   - `cargo test --all-targets --all-features` with zero failures and zero ignored tests
   - `cargo clippy --all-targets -- -D warnings`
   - metadata verifier, text/image fault gates, provider/id comparator, and manifest/crosswalk validators.
8. Use a local-first workflow. Finish implementation, reviewer/auditor corrections, docs, tests, and git hygiene locally before pushing. Hosted CI must run only once at the end for the final candidate; never use hosted CI as an iterative debugging loop. Batch fixes into the final candidate push unless explicitly told otherwise.
9. Final state must be clean/synced, Rui-authored, and non-rebased. Never use `git rebase`; use `git merge` / `git pull --no-rebase` if integration is needed.
10. Do not report completion until `RELEASE.md` and related audit evidence are current, all local gates pass, the one final hosted CI run has succeeded, and the branch is clean/synced.
