# Coding

## Change discipline

- Read relevant source, generated inputs, and existing tests before editing. Do not edit blind.
- Follow YAGNI principles and keep changes scoped to the requested audit/task.
- Preserve public API, serialization, provider behavior, and compatibility unless the upstream release explicitly changes them and the change is documented in `RELEASE.md`.
- Never hand-edit generated artifacts. Generated Rust catalogs and SBOM artifacts must come from pinned, reproducible inputs and committed scripts/tools.
- Generated source must be regenerated from the exact pinned upstream artifact/tag data used for the audit.
- Do not hide, weaken, or skip tests to make a gate pass. No unproven TODO completion claims.

## Git workflow

- Never use `git rebase`. Always use `git merge` / `git pull --no-rebase`.
- Commit as `Rui Carmo <rui.carmo@gmail.com>` unless explicitly told otherwise.
- Configure both local and global Git identity before committing:
  - `git config user.name "Rui Carmo"`
  - `git config user.email "rui.carmo@gmail.com"`
  - `git config --global user.name "Rui Carmo"`
  - `git config --global user.email "rui.carmo@gmail.com"`
- Final state must be clean and synced with `origin/main`.

## Official release discovery and bounds

For every future `@earendil-works/pi-ai` upstream release audit:

- Use the latest official published upstream tag/npm artifact and the exact prior accepted upstream tag as the only audit bounds.
- Never audit or diff against upstream `main` or untagged commits for release parity unless explicitly instructed.
- Verify and record the upstream tag SHA, npm package version, npm tarball SHA-256, and artifact provenance.
- Before implementation, mechanically derive and record:
  - exact upstream `git diff --name-status` path inventory and counts;
  - exact final upstream `packages/ai/test/*.test.ts` corpus;
  - full-record text catalog delta: old→new count plus added/removed/changed record counts;
  - full-record image catalog delta: old→new count plus added/removed/changed record counts.
- Keep release ledgers, generated-data metadata, and validators tied to those exact bounds.

## Coverage and evidence

- `RELEASE.md` is mandatory release-audit evidence for this repository.
- Each release audit must include an exact changed-path disposition matrix and a per-file upstream test crosswalk.
- Every applicable upstream delta must be covered through production Rust paths, not helper-only substitutes when transport/runtime behavior differs.
- Production-path coverage should exercise wire serialization, HTTP/SSE transport behavior, typed stream/parser behavior, replay semantics, raw/terminal errors, cancellation, usage accounting, and generated catalog metadata where applicable.
- N/A decisions must be narrow, explicit, and justified. Live-credential or hosted-only upstream cases must be separately labelled from deterministic local coverage.
- Record every Rust implementation, fix, adaptation, and N/A decision in `RELEASE.md` and related ledgers/crosswalks.
- Do not report a release parity audit complete until `RELEASE.md` and related audit evidence are current.

## Local gates and review

Run and record local gates before pushing:

- focused production-path tests for each changed behavior;
- `cargo fmt -- --check`;
- `cargo build`;
- `cargo test --all-targets --all-features` with zero failures and zero ignored tests;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- clean full-record text and image regeneration checks;
- independent deliberate text and image metadata fault gates;
- provider/id comparator equality;
- manifest/crosswalk validators;
- `make sbom-check`, `make license-check`, and `make vuln-check`.

Inspect diffs before committing, including generated-source drift and generated-data deltas. Resolve reviewer/auditor findings locally before the final push.

## Git and CI workflow

- Use a local-first workflow: finish implementation, reviewer/auditor corrections, docs, tests, regeneration, SBOM/security checks, and git hygiene locally before pushing.
- Batch fixes into the final candidate push unless explicitly told otherwise.
- Hosted CI must run only once at the end for the final candidate. Do not use hosted CI as an iterative debugging loop.
- If the final hosted run ID must be recorded after CI completes, use a docs-only `[skip ci]` commit or a proven `paths-ignore` mechanism so CI still runs only once for the runtime candidate. Record separate runtime and final-docs SHAs when this happens.
- Final release parity state must be Rui-authored, non-rebased, clean, and synced.

## Supply chain and SBOM

- `make sbom` must generate CycloneDX JSON plus SHA-256 checksum under the gitignored stable artifact directory:
  - `artifacts/sbom.cdx.json`
  - `artifacts/sbom.cdx.json.sha256`
- `make sbom-check` must validate schema-required fields, root crate identity/revision, non-empty direct+transitive dependency components, checksum correctness, and stale/malformed/empty artifacts.
- The SBOM generator is pinned by the committed `scripts/sbom.py` `GENERATOR_NAME`/`GENERATOR_VERSION` and the git revision. It consumes `cargo metadata --locked --all-features` and `Cargo.lock`; it must not embed secrets, absolute local paths, or volatile host data.
- SBOM artifacts are intentionally not committed. Commit only the generator, policy, workflow, and lockfile inputs.
- `Cargo.lock` is committed for reproducible Cargo resolution, SBOM generation, vulnerability scans, and CI. Do not hand-edit it; update it only through Cargo commands that correspond to `Cargo.toml` changes.
- `make vuln-check` runs the pinned RustSec scanner (`cargo-audit 0.22.2`) and must fail on high/critical advisories or warnings unless an owner-approved rationale and expiry are documented.
- `make license-check` reviews all resolved third-party licenses from Cargo metadata. Incompatible, unknown, or missing licenses require an owner, rationale, and expiry before acceptance.
- Any new dependency must include vulnerability and license review before the final candidate push.
- Final CI must generate, validate, scan, and upload the SBOM plus checksum artifact with retention. `RELEASE.md` must record SBOM tool/version, artifact path, digest, scan disposition, and license disposition for release audits.

## Lifecycle maintenance and triggers

Perform a lifecycle/security review when any trigger applies:

- new upstream `@earendil-works/pi-ai` official release;
- Rust dependency additions/removals/version changes;
- `Cargo.toml`, `Cargo.lock`, generated catalog, SBOM, CI, or release-script changes;
- RustSec or upstream security advisories, high/critical vulnerabilities, yanked crates, or license-policy changes;
- generated-data drift or provider catalog drift;
- public API deprecation/removal, serialization change, or compatibility-risking behavior change;
- release/tag/changelog publication;
- post-release verification failures, user/auditor findings, or rollback requests.

For lifecycle reviews, check Cargo manifest/lock consistency, regenerate/validate SBOM, run vulnerability and license reviews, inspect generated-data drift, update release/tag/changelog evidence, retain CI/SBOM artifacts according to workflow retention, and document rollback instructions to the last accepted SHA.

## Definition of Done

A release-audit or policy/SBOM task is not done until all applicable items are true:

- exact scope, upstream bounds, path matrix, and test crosswalk are recorded;
- runtime behavior is production-path tested with justified N/A/live-credential remainders;
- generated text/image catalogs are cleanly regenerated and deliberate fault gates prove drift detection;
- language gates pass with zero failures and zero ignored tests;
- strict all-feature Clippy passes;
- SBOM generation/check, vulnerability scan, and license review pass;
- `RELEASE.md` and related ledgers record final evidence, artifact paths/digests, scan/license disposition, and rollback/evidence pointers;
- exactly one final hosted CI run is green for the candidate unless an explicit exception is given;
- Git is Rui-authored, non-rebased, clean, synced, and the final SHA is reported.
