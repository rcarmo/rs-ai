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

1. Update `RELEASE.md` in the same commit as the release port/audit.
2. Record the exact previous accepted upstream tag/SHA and new upstream tag/SHA.
3. Record the exact audited upstream range and changed `packages/ai` path disposition matrix.
4. Record the JSON-shard-aware text/image comparator command and counts.
5. Record all Rust implementation/fix/adaptation details and all N/A decisions.
6. Record the Rust gates run and their results.
7. Do not report completion until `RELEASE.md` is current for the release.
