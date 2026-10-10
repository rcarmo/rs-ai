# rs-ai upstream release parity

## v1.1.0 implementation checkpoints

The v1.1.0 upgrade is in progress. Local checkpoints are pushed with `[skip ci]`; final hosted CI, clean-clone gates, profiling, full contract dispositions and release acceptance have not run. Existing v1.0.1 runtime tags and publication receipts are unchanged.

- Official source range: `a7229ddc21810d6245105978033b7df645ecc2f7..abe508e1b89912adde45528136c3221eb69acdd7`.
- Artifact pins and exact inventories: [pi-ai crosswalk](docs/v110-upgrade-crosswalk.md) and [pi-durable crosswalk](docs/pi-durable-v110-crosswalk.md).
- Portable path-policy checkpoint: `c1b00a471a713140640fc58ff04799e95c87ef44`, pushed to `origin/main`. Shell syntax, Python AST, local/CI fallback resolution and invalid-override rejection passed. Runtime changes were kept separate.
- Runtime/catalog checkpoint: Rust 1.99.0; `cargo test --locked --no-default-features --lib` passed 1,035 tests; `cargo test --locked --lib` passed 1,192 tests, with zero failures or ignored tests. Strict `cargo clippy --locked --all-targets --all-features -- -D warnings`, format and diff checks passed.
- Pinned offline metadata comparison and double regeneration passed: 1,563 chat, 61 image and 26 classifier records; 1,650 total, 74 batch aliases. Historical regression suites still check retained behaviour; their live-catalog counts and Azure identity follow v1.1.0. Historical fixture delta expectations are unchanged.
- Implemented slices include UTF-16 token estimates, thinking-aware sampling, retry classifications, Decisions classifier/image pricing, Azure request configuration, assistant/execution timing, Anthropic callback fallback, ChatGPT login identity, Radius replacement catalogs and durable provider-session identity/lifecycle timestamps.
- OAuth rotation checkpoint: `refresh_stored_oauth_credential` cancels lock waiting but lets admitted refreshes persist before releasing the lock, even after caller cancellation/drop. The worker has an independent 15-second timeout. Pre-cancelled resolution starts no provider work; closed cancellation channels do not cancel. Only explicit minimum-validity requests impose a post-refresh expiry requirement. `ProviderAuth.oauth` now takes `Arc<dyn OAuthAuth>` instead of `Box<dyn OAuthAuth>` so the worker owns the provider; credential-store clones share state and locks.
- OAuth evidence: 36 focused auth tests and six real-provider adapter tests passed, including one-refresh persistence after cancellation for Anthropic, Codex, Kimi, Xai and Radius. Default-feature library tests passed 1,198/0 with zero ignored tests; strict all-feature Clippy, format and diff checks passed. The timeout test exercises the real 15-second bound. Model-catalog refresh now uses the shared credential helper before network source work; offline refresh does not rotate. Cancellation and superseding refreshes preserve admitted rotations and suppress stale source calls. Radius runtime providers expose their OAuth handler. The 17 runtime-refresh tests and default-feature library suite passed 1,201/0; strict all-feature Clippy passed.
- Codex transport headers now merge defaults, model headers and request headers case-insensitively on both SSE and WebSocket. Hosts can override `originator` and `User-Agent`; bearer authorization and account identity come from the selected credential. Request-header null deletion is not expressible by Rust's string-valued map. SSE wire and WebSocket handshake checks pass; 49 focused Codex tests and the default library suite passed 1,202/0, with strict all-feature Clippy.
- Provider regressions now cover Haiku-5 Bedrock adaptive/xhigh/binding/cache eligibility, flat GPT-OSS and nested GPT reasoning fields, and Mistral terminal server-error messages with retained text/usage. The Bedrock checks exercise the production field builder; the Mistral check uses HTTP/SSE. Default-feature library tests passed 1,205/0; strict all-feature Clippy passed. Live Bedrock requests have not been verified.
- Durable snapshots now offer ordered entry/task/submission pages with detached values, exclusive continuation IDs, direction-preserving cursors and conflict rejection. Entries default descending; tasks/submissions ascending. Tests cover both directions, legacy cursors, conversation isolation and rejected-commit atomicity. Backend streaming/conversation scans and SQLite remain gaps; the later filtered session-query checkpoint adds native range/kind/status filters. Durable tests passed 63/0, default-feature library tests 1,207/0, and strict all-feature Clippy passed.
- Durable historical text context now accepts `ContextOptions { at }` through `context_with_options`, retaining the existing no-argument `context()`. Inclusive cuts validate visibility and perform no mutation or dispatch. Submission tests cover passive/user/answer/tail cuts; default library tests passed 1,208/0, strict all-feature Clippy passed. Tool/edit/head/fork context contracts are absent from the native subset.
- Model-refresh publications now serialize across attempts and provider replacements. In-memory catalogs update only after storage settles; admitted writes finish on cancellation/drop, while stale callback writes/deletes are fenced. Replacement, deletion and clearing supersede pending sources; dropping a refresh aborts its source task. Seven new race tests plus the prior runtime suite pass 24/0; default library tests pass 1,216/0 with strict all-feature Clippy. Publication storage has no independent timeout, matching the caller-cancellable upstream wait; a non-settling custom store can hold later publications.
- Durable task lifecycle stamps accept an injected `LifecycleClock` through `DurableSession::open_with_clock` and `DurableHarness::open_with_tools_and_clock`; existing constructors keep wall-clock defaults. The clock is called only for missing start/end stamps. Three added tests verify exact timestamps, recovered starts, negative-time atomic rejection and harness forwarding. Library tests passed 1,219/0 with strict all-feature Clippy; execution durations stay monotonic and independent of this clock.
- Targeted durable commit profiling (Rust 1.99.0 debug tests, Valgrind 3.27.1) used the same two 128-entry workloads for memory/journal storage before and after removing whole-snapshot staging clones and journal revalidation. Callgrind instructions fell 376,821,995 -> 222,933,397 (40.84%); Memcheck allocation count 77,500 -> 7,996 (89.68%), allocated bytes 20,679,609 -> 2,561,609 (87.61%); Massif peak heap 339,289 -> 334,345 bytes. These are instrumented development-workload measurements, not release throughput or complete pre-release profiling. Both Memcheck runs reported zero errors; leak classification was disabled. Validation precedes all mutation, journal append/sync/uncertain fencing and detached loads are unchanged. Raw captures, annotation files and the matching profiled test binary were deleted after analysis. Default-feature library tests passed 1,221/0 and strict all-feature Clippy passed after the optimisation; backend conformance also checks invalid final records leave the entire snapshot unchanged.
- Radius regression coverage now verifies remote/cached empty organization catalogs replace shipped defaults and cached nonempty catalogs expose only configured identities. Twenty-six runtime-refresh tests and the default library suite passed 1,223/0; strict all-feature Clippy passed.
- Assistant terminal timing now covers exported provider stream entry points as well as registry dispatch, using nearest-millisecond monotonic rounding. Supplied durations and older forwarded messages remain unchanged. Faux text streams use current creation timestamps. Four duration tests include direct HTTP/faux paths and rounding boundaries; default library tests passed 1,225/0 with strict all-feature Clippy. Rust has no upstream EventStream subclass/end(result) API.
- Shared classifier cancellation now spans success/error response-body reads after headers, not just request sending/retry waits. A real TCP fixture stalls a Decisions JSON body and verifies prompt aborted completion without answers. Seven Decisions tests and default library tests passed 1,226/0 with strict all-feature Clippy. Custom fetch and fractional token counts have no native equivalent.
- Broader development gates on the pushed classifier/timing checkpoint passed 1,226 all-target/all-feature tests, one doctest and 1,066 no-default library tests with zero failures/ignored tests. Durable normal/recovered tool attempts now share upstream nearest-ms monotonic rounding; 69 durable tests and four duration tests passed with strict all-feature Clippy after the correction.
- Azure shared configuration trims explicit resource/version/deployment values and falls back for whitespace-only options, matching official v1.1.0 configuration precedence. Added regression covers environment and explicit whitespace cases. Twenty-six Azure tests and library tests passed 1,227/0; strict all-feature Clippy passed.
- Development supply-chain checks on `47e44d0` generated/validated an SBOM with 270 third-party components, passed all 270 licence reviews, 10 licence self-tests and eight vulnerability-policy self-tests. Installed pinned `cargo-audit 0.22.2` under the project-owned tool root; the actual vulnerability scan passed. Dependency versions are unchanged. Disposable scanner build output and scratch SBOM were removed after review; these are local development results, not v1.1.0 hosted/release assets.
- Sampling precedence now has public HTTP-dispatch regression coverage across OpenAI Completions, Responses and Azure Responses, in addition to payload-builder/clamped-level tests. Five sampling tests and the default library suite passed 1,228/0 with strict all-feature Clippy.
- Shared classifier response hooks fire on received HTTP responses before body parsing, including non-success and malformed JSON. A regression verifies status/headers/model observation for HTTP 200 malformed JSON and HTTP 400. Eight Decisions tests and the library suite passed 1,229/0 with strict all-feature Clippy. Native retry hooks observe the final response; per-attempt/custom-fetch hooks require further adaptation.
- Fixed the path-policy integration mismatch: CI SBOM uploads, publisher provenance reads/assets and direct `scripts/sbom.py` defaults now use the resolved project-owned build/run roots. Source-local `artifacts/`, `dist/` and `.runtime-ref` are no longer runtime output paths. Both workflow YAML files and all publisher shell blocks parse; direct default SBOM generate/check passed for 270 components. Path regression and library tests passed 1,230/0 with strict all-feature Clippy. No publisher or hosted workflow was dispatched; local scratch was deleted after validation.
- Native tools now receive a shared host `DurableModels` service for lookup/completion/classification via `ToolExecution.models`, including recovered executions. Default constructors use `RegistryModels`; `open_with_tool_models` injects the application object without persisting it. Three regressions cover identity/nested completion, recovered current host/no-open-dispatch and default HTTP completion/Decisions. Library tests passed 1,233/0 with strict all-feature Clippy. Direct ToolExecution construction needs the new `models` field. Hooks/generic tasks, progress-output windows and automatic nested-call usage accounting are unimplemented.
- A clean `--no-local` Git checkout of `434a3bb` passed 1,233 all-target/all-feature tests, one doctest, strict Clippy, format and exact inventory structure checks, with clean status before/after. It reused the project tool/build cache; this verifies checkout isolation, not a cold dependency rebuild. The disposable checkout/run was removed after use.
- System One now uses the v1.1.0 shared classifier request/usage path instead of its duplicated transport/flat-price parser. Added HTTP regressions verify prompt-length tier prices and cancellation after headers on a stalled body. Five System One tests and library tests passed 1,235/0 with strict all-feature Clippy; malformed answers retain billed usage.
- Durable filtered pages now run on the session mutation queue through `entries/tasks/submissions`, with harness counterparts. Entry queries support inclusive ID bounds/kind and BTreeMap range seeking; task queries kind/state/abort; submission queries status. Results are detached, wait for admitted commits and reject on poison/close. Three new regressions cover query filters, ordering and harness results. Durable tests passed 75/0, library tests 1,238/0, and strict Clippy passed with/without default features. Also fixed an unhandled TCP fixture read amount exposed by the no-default strict lint gate. Fork/background/conversation scans and SQLite are still absent.
- Durable tool-call assistant messages now persist with pending children before effects. Native context reconstruction retains those rounds for later submissions/recovery, orders results by call, preserves durations and synthesizes missing results without dispatching work. `message_context(ContextOptions { at })` exposes inclusive native reads; legacy `context()` stays text-only. Four new tests plus recovery assertions cover order/orphans/missing results, later submissions and queued FIFO input IDs. Durable tests passed 79/0; library tests 1,242/0; strict Clippy passed with/without defaults. Fork/edit/head/full open-message/cache contracts are unimplemented. New journals contain assistant tool-round entries; old text records remain readable.
- Durable model runs now carry optional terminal `duration_ms`; completed initial/successor answer entries persist it as `durationMs`, and native context preserves it. Registry-runner HTTP and successor assertions verify timing propagation; a supplied-duration journal reopen test reads 1,234 ms without dispatch. Library tests passed 1,243/0; strict Clippy passed both configurations. `ModelRun` direct construction needs the optional field; `ModelRun::one` keeps untimed defaults. Aborted/error paths do not fabricate answer durations.
- Native current-message context is now derived/cached on the session line, with ten-minute sliding idle retention, expiry timer, zero mode and successful-commit/poison/close invalidation. Historical cuts bypass it; returned messages are detached. Cache bounds are 64 conversations, 4 MiB encoded context each and 32 MiB total, excluding object overhead. `HarnessServices` provides a settings-bearing constructor. Two new regressions verify reuse/isolation, commit invalidation, historical bypass and expiry/zero. Library tests passed 1,245/0; 82 durable tests and strict Clippy both configurations passed. Incremental appended-range reuse, busy retention and fork/edit/head semantics are not implemented.
- Native context excludes assistant stop reasons `aborted`, `error` and `deferred`, then removes orphan results through call ordering. New tests cover excluded history/foreign cut and malformed user/assistant/tool records without mutation; cache tests confirm rejected commits keep valid cached context. Durable tests passed 84/0; library tests 1,247/0; strict Clippy passed both configurations.
- Idle-only `ContextUpdate` entries now provide native head markers and omit/replace edits. Latest visible heads/edits determine both native and text-compatible context, historical cutoffs limit updates, leading system metadata moves before user-only prefixes, and storage validates prior/same-conversation targets and edit shapes before mutation. Three new regressions plus busy-fence assertions cover updates, reset/reopen/next intent and atomic invalid references. Durable tests passed 87/0; library 1,250/0; strict Clippy both configurations passed. Busy-boundary scheduling, forks and generic upstream EntryDraft remain unsupported; native `context` entries extend the journal vocabulary.
- Targeted native-context profiling (Rust 1.99.0 debug, Valgrind 3.27.1) compared the same 64 reads of 128 text messages. Removing text-only result-ordering deep clones reduced uncached instructions 136,285,308 -> 93,507,870 (31.39%), allocations 53,589 -> 36,821 and allocated bytes 34,631,109 -> 18,968,005. Bounded size accounting now uses a counting writer instead of a temporary JSON buffer. Final cached reads used 92,990,244 instructions, 20,258 allocations and 10,894,477 bytes versus optimised uncached 93,507,870/36,821/18,968,005; instruction difference is only 0.55%, allocated bytes 42.56% lower. Cache peak heap is higher: 638,868 vs 490,668 bytes. All Memcheck runs report zero errors with leak classification disabled. This small instrumented workload does not establish release throughput; full pre-release profiling is open. Raw captures/annotations and matching binary were removed after analysis. New marker-edge and repeat-read tests pass; library 1,253/0 and strict Clippy both configurations pass.
- Native committed-state watches attach snapshot/subscription atomically on the session queue, publish only adopted batches, and terminate on stop/drop/close/poison. Bounded queues replace overflowed deltas with current snapshots; limits are 64 watches and 64 records/32 MiB encoded deltas per queue, excluding snapshot/object overhead. Three new regressions plus uncertain-settlement assertions verify no-before-adoption/no-rejected-event, overflow reconciliation, detach/cap and poison. Durable tests passed 93/0; library 1,256/0; strict Clippy both configurations passed. Full upstream AgentEvent/transient-progress/acquisition-signal/listener semantics are unimplemented.
- Watch registration now has an RAII owner that ends receivers as poisoned if the session worker unwinds. A panic-injected clock regression verifies pending readers terminate, return one End and do not hang. Four watch tests and library tests passed 1,257/0; strict Clippy both configurations passed.
- Added `verify_v110_baseline_delta.py`: exact integrity-pinned v1.0.1/v1.1.0 artifact extraction, schema checks and typed full-record deltas (chat 1536->1563 +79/-52/199, image 59->61 +2/-0/0, classifier 20->26 +6/-0/1). Separate unchanged-record metadata faults reject for all three catalogs; legacy Azure identity is rejected. Offline cached-artifact and normal npm-spec regressions passed. Library tests passed 1,258/0; strict Clippy both configurations passed. Disposable extraction/run output was deleted after validation.
- Source-bound context review corrected `selectActive` ordering: the newest head marker contributes before retained non-head entries; older markers contribute only their in-range edits. Tool-result repair now precedes leading-system promotion, so an orphan result cannot hide the baseline system prefix. Missing-result text matches the pinned upstream contract. Three added regressions cover head ordering/omit, reset-range edits and orphan/system/excluded replacement messages; library 1,261/0 and no-default library 1,099/0 passed, with strict Clippy in both configurations. This does not close fork, incremental-range or busy-retention parity.
- Added native `ContextView` reads to session/harness: detached resolved head marker, active entries, aligned per-entry contributions and repaired messages. Omitted/model-less entries retain empty contributions; synthesized results affect only final messages. Shared derivation avoids full-view entry/contribution clones for messages-only callers; a subsequent profile-guided pass removes the scratch contribution buffer entirely. Empty/current/historical/isolation/close, harness reset, admitted-commit ordering and poison checks passed; all-feature targets 1,262/0, doctest 1/0, no-default library 1,100/0 and strict Clippy both configurations passed. Initial fixture failures (same-batch prior reference, missing user text) were corrected without weakening validation. Generic upstream EntryRecord/forks/incremental retention remain open.
- Targeted context-view refactor comparison used Rust 1.99.0 debug tests and Valgrind 3.27.1, 64 reads of 128 text entries, cached and uncached, against `1b30a6d`. The first refactor added uncached scratch work (101,729,731 instructions / 36,887 allocations / 19,191,992 allocated bytes); direct append removes it (97,948,995 / 36,823 / 18,995,382), effectively matching baseline (97,999,393 / 36,823 / 18,995,233). Peak uncached heap: baseline 491,585, first refactor 494,000, tuned 491,582 bytes. Cached tuned/baseline instructions 93,905,595/93,898,056; both 20,260 allocations, allocated bytes 10,921,846/10,921,697, peak heap 637,782/640,345. All six Memcheck runs reported zero errors with leak checking disabled. Inclusive CPU review finds native text conversion/serialization and detached Message cloning dominant; correctness requires caller isolation. This is allocation-regression removal in a small diagnostic workload, not release throughput or full-view performance evidence. Raw profiles, clones, logs and matching binaries were deleted immediately after analysis. Tuned all-feature targets 1,262/0, doctest 1/0, no-default library 1,100/0 and strict Clippy both configurations passed.
- Fresh catalog regeneration and all 1,650 typed records verified at `03c95bc`; SBOM/checksum validation and license review covered 270 dependency components, ten license/eight vulnerability self-tests passed, and actual cargo-audit 0.22.2 passed. Disposable SBOM and verification output were removed after review; no accepted v1.1.0 SBOM is published.
- Fresh `--no-local` clean-source clone at `837fb12` passed all-feature targets 1,262/0, doctest 1/0, no-default library 1,100/0, both strict Clippy configurations, format and clean-tree checks. This reused the project Cargo cache/target (not a from-scratch dependency build). Inventory structure passed; separate `--require-complete` checks failed as expected for AI 448 and durable 215 unresolved crosswalk rows. The clone and disposable logs were removed. These rows are unreviewed contract mappings, not missing-feature counts; no hosted acceptance or release publication ran.
- Added generic passive `EntryDraft` and session/harness `append_entry`: custom kinds, optional JSON data/model messages, self/prior heads and edits. Identity allocation runs on the session line; admitted settlement shares commit/watch/poison semantics and returns only the detached entry without cloning a whole reply snapshot. Explicit null data differs from omission. Eight new generic/lifecycle tests cover model-less context, custom head/edit contributions, concurrent allocation, invalid/oversized/deep payloads, prior/foreign references, raw payload rejection, journal reopen/provider forwarding, tombstoned-before-admission and admitted caller drop/poison; the harness busy fence is also tested. All-feature targets 1,270/0, doctest 1/0, no-default library 1,108/0 and strict Clippy both configurations passed. Generic fields live inside native EntryRecord.value; typed tokens, conversation existence/forks, scoped task attribution, generic transactions and busy-boundary writes are unimplemented.
- Entry reference validation now includes earlier writes in the same ordered CommitBatch, matching transaction-local append visibility. The overlay retains references only to the current batch instead of cloning/indexing the whole snapshot. A memory/journal regression covers generic head/edit and native context edits targeting earlier batch entries, rejects reordered/foreign batches atomically, and verifies journal reopen. Durable 107/0, all-feature targets 1,271/0, doctest 1/0, no-default library 1,109/0 and strict Clippy both configurations passed. Full generic transactions and conversation/fork existence checks are open.
- Idle harness `update_context` and generic `append_entry` now reject stale numeric heads before allocation, matching upstream `harness/inbox.ts::isStale` and spec admission policy. Targets below the newest retained head cannot restore dropped history; equal/self heads pass. Raw session history batches keep their separate policy. Existing context-update regression verifies both stale APIs, unchanged state, equal/self admission and no provider dispatch; durable 107/0, all-feature targets 1,271/0, doctest 1/0, no-default library 1,109/0 and strict Clippy both configurations passed. Queued-write unanswered/stale settlement is not implemented.
- Added session/harness `entry` lookup on the mutation queue: one detached record, None for missing/foreign native-conversation IDs, no whole-state clone. Existing generic/journal and lifecycle regressions now verify isolation, reopen, read-after-admitted-commit and poison/close. All-feature targets 1,271/0, doctest 1/0, no-default library 1,109/0 and strict Clippy both configurations passed. Fork-aware visibility and typed-token filtering are absent.
- Session-wide `tasks_in`/`submissions_in` accept omitted conversation filters, matching upstream query scope; existing APIs/harness remain conversation-scoped. Shared detached scans retain filters and exclusive direction-preserving cursors. A two-conversation regression covers global descending-task/ascending-submission pages, cursor conflict, filters, unchanged state and close; poison assertions extend lifecycle coverage. Initial fixture ID expectations were corrected to the existing helper's IDs. All-feature targets 1,272/0, doctest 1/0, no-default library 1,110/0 and strict Clippy both configurations passed. Background task flags, conversation-record scans and backend/SQLite parity are open.
- Targeted entry profiling compared `9a3d560` with a conditional batch-reference overlay, using Rust 1.99.0 debug tests/Valgrind 3.27.1. Equivalent 128 growing memory commits: instructions 82,537,075->82,213,943 (0.39%), allocations 4,189->4,061 (128 fewer), allocated bytes 1,375,090->1,364,335, peak heap 324,884->324,881. Generic 32-concurrent-appends control retained 3,140 allocations, bytes 964,992/964,989 and instructions 15,156,244/15,164,742 (no measured improvement). Inclusive CPU review shows validation/serialization dominant; skip overlay allocation only for native batches with no context-reference entries, without skipping validation. All four Memcheck workloads had zero errors with leak checking disabled. An initial wrong test filter ran zero tests and was rejected; its probe output was deleted immediately. All raw profiles/logs/matching binaries were deleted after analysis. Final all-feature 1,272/0, doc 1/0, no-default 1,110/0 and strict Clippy both configurations passed. These small diagnostics are not final release throughput/profile coverage.
- Added process-local `EntryDefinition<D>` tokens: validated custom kind, typed required-data draft creation, identity-only `matches`, and Serde `decode` with None for absent/foreign records and errors for matching malformed/missing data. Nullable data round-trips; no registry/schema is persisted. Token cloning does not require D:Clone. New regression covers equal-kind independent tokens, missing/invalid/null data and no persistent definitions. All-feature targets 1,273/0, doctest 1/0, no-default library 1,111/0 and strict Clippy both configurations passed. Scoped attribution, generic transactions and flat upstream record encoding remain open.
- Current-message reads now retain a settled prefix/raw open suffix and seek/decode only new entries; late tool results repair missing-result placeholders. Heads/edits rebuild, changed native task input invalidates, unrelated and lifecycle/checkpoint commits retain ranges. Historical/full-view reads bypass retention. Four regressions cover appended decode counts/full-derivation equivalence through tool results/heads/edits, unrelated/task-input commits, growing reads and conservative budget overflow. All-feature targets 1,277/0, doctest 1/0, no-default library 1,115/0 and strict Clippy both configurations passed. Generation-task, historical/full-view reuse and busy-task pinning are absent.
- Range profiling used Rust 1.99.0 debug/Valgrind 3.27.1 against `dda964b` with the identical new 128-entry append/read test added to its disposable clone. Initial full-range sizing regressed instructions to 2,229,528,905 despite lower allocations; tuning sizes additions only under a conservative saturating retention budget. Baseline->tuned growing workload: instructions 1,633,406,547->374,217,602 (-77.09%), allocations 329,806->182,313 (-44.72%), bytes 81,619,136->49,559,193 (-39.28%), peak heap 1,156,634->1,314,011 (+13.60%). Same 64x128 cached-read control: instructions 93,953,649->95,131,287 (+1.25%), allocations 20,260->20,519, bytes 10,929,690->11,178,103, peak heap 646,458->796,167 (+23.16%). Raw+assembled retention trades memory/startup cost for growing-read CPU/allocation savings; inclusive review found storage/JSON cloning and validation still dominant. All six Memcheck runs had zero errors with leak checking disabled. Raw captures, clones, matching binaries and logs were immediately deleted after analysis. These debug diagnostics are not final release throughput/full-view profile coverage.
- Source-bound native busy retention now follows scheduler idle semantics: any nonterminal same-conversation task pins a current-message range, terminal adoption starts its idle deadline, reads do not slide expiry, and zero retention drops idle ranges while allowing busy reuse. Two regressions verify zero/short pinning through pending/running/terminal transitions and a non-sliding deadline. The first fixture attempted an invalid pending->succeeded transition; it was repaired to use running admission without weakening storage validation. All-feature targets 1,279/0, doctest 1/0, no-default library 1,117/0 and strict Clippy both configurations passed. Generation-task/full-view/historical reuse and background/owned-conversation semantics are open.
- Contract review still includes remaining provider cases and broader durable storage/context semantics. The durable R1 vertical retains its documented baseline gaps.

`scripts/validate_v110_manifests.py` verifies the pinned hashes, cardinalities and ordered crosswalk rows for both packages. AI inventories contain 82 changed paths, 197 source paths and 174 test/support paths (167 executable tests); durable inventories contain 59/67/90 paths (49 executable tests). `--require-complete` fails with unresolved rows, separately from structural validation. Inventory/crosswalk fault sentinels fail for both packages. The full library suite passed 1,209/0 and strict all-feature Clippy passed after adding the validator regression. Source/test rows are tracked independently and duplicate references count separately in unresolved totals.

These are development checks. No v1.1.0 release or parity acceptance is recorded.

## Accepted v1.0.1 audit

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v1.0.1`
- Upstream tag/npm gitHead: `a7229ddc21810d6245105978033b7df645ecc2f7`
- Previous accepted upstream: `v1.0.0` / `a13d35a742c6ef8462812a28fbe1d8c8b7431c32`
- Implementation baseline: `0be75ef2eb571644eb4eeabb2cee19d4bc7543f6`
- Official range: `a13d35a742c6ef8462812a28fbe1d8c8b7431c32..a7229ddc21810d6245105978033b7df645ecc2f7`
- Artifact SHA-256: `8a9e69b1309cf93405d87729fa123c8b11c6be7c646b16f34f8bef7b792f9138`
- npm integrity: computed SHA-512 matches registry metadata; npm advertises SLSA v1 provenance, but this cycle has not independently verified its signature/transparency record
- Scope status: **USEFUL DURABLE R1 NATIVE AND UPSTREAM-ALIAS REPLACEMENT PUBLISHED; FULL PI-DURABLE PARITY IS NOT COMPLETE**.

The exact upstream delta contains **19 changed paths**, `+546/-153`, and **6 changed tests**. The final corpus contains **171 test/support paths**: 164 executable tests and seven support files. `scripts/validate_v101_manifests.py --require-complete` verifies exact manifests and the complete crosswalk with no pending rows.

The native runtime uses Anthropic inline full tool definitions from the first eligible request, a stable initial tool/placeholder cache prefix, same-name replacement and outer update-block cache control. Bedrock adds binding controls only for eligible non-GovCloud adaptive Claude models. Cloudflare System One accepts direct `answers` and nested completed-run envelopes. Assistant retry classification includes `model is at capacity` while quota/billing precedence remains non-retryable. ChatGPT OAuth now exposes a host-owned cancellable login API, binds port 1455 before host effects, preserves a typed internal bind cause, claims callback/manual exchange once and joins owned prompt/listener work on completion or cancellation.

All schema-v6 catalogs are regenerated offline from the verified pinned artifact: **1536 chat**, **59 image** and **20 classifier** records, **1615 total**, across 42 provider files. The full-record v1.0.0→v1.0.1 deltas are chat `+17/-13/54 changed`, image `+2/-0/0 changed`, classifier `+5/-0/0 changed`. An independent export through the public native registries compared every typed identity and all metadata against the official 42 shards with zero semantic differences. Two established native representations are retained: chat `type` is implicit in Rust `Model`, and Serde's native `supportsOpenaiGrammarTools` field maps to upstream `supportsOpenAIGrammarTools` with the same typed value. The manifest structure hash is `03d2e1aeeee6eb16959d4f727b47b9b187efaf863c688a47889fb90d200e6812`. Generated Rust SHA-256 receipts are chat `dcb3da25d8ece8de321544278194c05dfdd743963492dea3e517c4a99dc4d231`, image `b083239189ddf81097805ecd0fdaa910fc04a1ebf726e55c269cdf4e35e13832`, classifier `84ecedf4b733f543a1838e9c57b0e37e6aaecf8f70b13da424f8aed0fc9e49ab`; hashes record provenance only. Acceptance uses full-record semantic comparison and deterministic regeneration, not output-hash equality.

Staged hydration validates the exact artifact, complete schema/provider/API/cost/modality identities and all three rendered/formatted outputs before replacing accepted files. Validate-only and normal render/format fault sentinels leave accepted outputs unchanged. No dependency, AWS graph, workflow, Makefile or publisher-policy change belongs to this runtime.

Local evidence to date: focused provider/retry/release tests passed 29/0 before OAuth lifecycle expansion; the ChatGPT OAuth, shared callback and Anthropic OAuth suites passed 8/0, 5/0 and 11/0 after the public-cancellation correction. An independent public API probe reports `Login cancelled`, prompt cleanup complete before return and successful port-1455 rebind. No-default tests passed **947/0**; all-target/all-feature and explicit Bedrock matrices each passed **1104/0**, with no ignored tests. Format, build, strict all-target/all-feature Clippy, exact manifests, full-record mutation faults, metadata equality, double generation, staged hydration faults, licence and vulnerability gates passed. Security includes 270 third-party packages, licence/vulnerability self-tests 10/10 and 8/8, zero cargo-audit vulnerabilities, and SBOM SHA-256 `8b804f05479945755be86f6cc95b15922649463bacdeeba7c6c5ad3baccf46bd`.

The final local scope was 48 paths: 37 tracked modifications plus 11 new files, `+1786/-512`. The sorted status inventory SHA-256 was `dc4422a17b245256152d81fd70bb02d0ab9831ff0136c669415db2986bfe7b34`. A real detached worktree from baseline `0be75ef2eb571644eb4eeabb2cee19d4bc7543f6` passed exact manifest validation, strict hydration valid/fault sentinels, format and all-target/all-feature compilation. Final ChatGPT lifecycle controls passed **11/0**, including deterministic callback/manual single exchange, joined browser request, present/prompt/parse error cleanup, caller cancellation cleanup-before-return, dropped-public-future listener release and port rebind. The final shared callback and Anthropic OAuth suites passed **5/0** and **11/0**; strict Clippy passed after those changes.

### v1.0.1 initial hosted acceptance and publication

- Accepted runtime: `73de29d00230f794cac08b71bd669ca9386772db`
- Runtime tree: `5202f38da904a8ed0060a286eeadf03ad5acb452`
- Parent/rollback: `0be75ef2eb571644eb4eeabb2cee19d4bc7543f6`
- Normal push CI: `37161895463`, attempt 1, job `111316886118`; all hosted steps passed
- Hosted test result: **1107 passed, 0 failed, 0 ignored**
- SHA-specific artifact: `rs-ai-sbom-73de29d00230f794cac08b71bd669ca9386772db` (`11288450921`)
- Artifact archive SHA-256: `b8909f7566e3a84b91e4bb7953dd1f4ceb9afb7550a61b4365761ab5115728d9`
- SBOM SHA-256: `3ef55884fdaec88daf384028d08e5d3854c90311e6bb9f7511c7e9c2f07bb2d8`
- Checksum-file SHA-256: `f6e04e7808cd4785bbad6b7a8cecc22fad5c49e93fa76a73fe04441542c2cd16`
- SBOM root: `rs-ai@1.0.1`, exact runtime revision, 270 components, 271 dependencies and 23 direct root edges
- Post-commit licence and vulnerability gates passed with zero cargo-audit vulnerabilities or warnings

Native publication:

- Annotated tag: `v1.0.1`
- Rui-authored tag object: `4110c49168ca52f4f7e3af07f2902d57a18f91bd`
- Peeled runtime: `73de29d00230f794cac08b71bd669ca9386772db`
- Publisher run: `37162332446`, attempt 1, job `111318164299`; all 13 steps passed
- Release: `https://github.com/rcarmo/rs-ai/releases/tag/v1.0.1` (`402735140`)
- Assets: `sbom.cdx.json` (`608731681`) and `sbom.cdx.json.sha256` (`608731679`)

Upstream-alias publication:

- Lightweight tag: `upstream-v1.0.1`
- Direct runtime target: `73de29d00230f794cac08b71bd669ca9386772db`
- Publisher run: `37162431527`, attempt 1, job `111318459944`; all 13 steps passed
- Release: `https://github.com/rcarmo/rs-ai/releases/tag/upstream-v1.0.1` (`402735666`)
- Assets: `sbom.cdx.json` (`608734300`) and `sbom.cdx.json.sha256` (`608734301`)

Both releases contain exactly the canonical SBOM and checksum assets. Public downloads validate and are byte-identical to the accepted hosted artifact. All 25 older tag refs and 14 older releases/assets remained unchanged.

The initial v1.0.1 publication above is retained as historical evidence. Its annotated tag object `4110c49168ca52f4f7e3af07f2902d57a18f91bd`, runtime `73de29d00230f794cac08b71bd669ca9386772db`, release IDs and initial asset IDs/digests remain the rollback and preservation record for the same-version replacement.

### v1.0.1 durable useful-runtime replacement

The accepted runtime is `12ce97be283aebf3ded6764b7c796785b8b7781a`, with tree `c6c738b61f8413910d18c3dd931a7533610b19f3`. The durable sequence consists of R1a `fc754aea5622a1c93e083f795989ec97b1d7fe8b`, R1b `84f4152889ed3a1d665428b7287164d50814cb66` and the accepted R1c checkpoint `1189c92ee9e5dbf3681d63ec4331c78a9062261d`. R1a adds the storage/session kernel; R1b adds persistent root generation; R1c adds the model→owned tool→result-aware answer path with durable intents, stable idempotency keys, exact safe-replay gates, cumulative usage, explicit recovery, bottom-up abort and close drains.

Local R1c acceptance covered focused durable tests **60/0**, no-default tests **1010/0** plus one doctest, all-feature tests **1167/0**, and explicit Bedrock tests **1167/0** plus one doctest. Format, build, strict Clippy for all-feature and no-default profiles, an isolated Rust 1.85 durable-module shape check, SBOM, licence, vulnerability, manifests, full-record metadata, deterministic catalogue generation, hydration and fail-closed fault gates passed. The unchanged whole dependency graph retains its Rust 1.88 floor; the isolated check is not a whole-crate Rust 1.85 compatibility claim. A genuine detached Git clone passed the corresponding source-bound gates. Synthetic validation commits `a934eb7719d82f94b292aaacdf8c91fb34035a7a` and `d07f67211106f479980bddfb4b9b9b6c35e0672a` were never release targets.

The first hosted R1c run, `37176785519`, failed strict Clippy under Rust 1.99 on `clippy::needless_late_init`; no test, SBOM or security validation steps ran after strict Clippy failed, while checkout, toolchain setup, cache, pinned cargo-audit installation, format and build passed. The run produced no artifact. The one-block mechanical correction is commit `12ce97be283aebf3ded6764b7c796785b8b7781a`, parent `1189c92ee9e5dbf3681d63ec4331c78a9062261d`. Isolated Rust/Clippy 1.99 local and clean-clone validation passed before the successor push. Normal push CI run `37177760346`, attempt 1, job `111363862250`, passed all 18 steps with **1167 passed, 0 failed, 0 ignored**. SHA-specific artifact `rs-ai-sbom-12ce97be283aebf3ded6764b7c796785b8b7781a` (`11293778981`) has archive SHA-256 `90ac7a9be586bd1192bb9beb333764382b0b7340d901a261dfd225cb34aa9439`.

The accepted CycloneDX 1.5 SBOM has root `rs-ai@1.0.1`, embedded revision `12ce97be283aebf3ded6764b7c796785b8b7781a`, 270 components, 271 dependency records and 23 direct root edges. Its SHA-256 is `e2447de15d942f1be413f33dde4f728fede32474f2de993d1e473c2039636e0b`; the checksum-file SHA-256 is `1af5805bae5b1f343124a3234af80964632feec6b26025d80b6d6b872dbf8b09`.

Native same-version replacement:

- annotated tag `v1.0.1`, Rui-authored tag object `6d5717e64970feef066b5d2f34b1a2b6e966975a`, peeled runtime `12ce97be283aebf3ded6764b7c796785b8b7781a`;
- publisher run `37178159190`, attempt 1, job `111365041221`; all 13 steps passed;
- release `402735140`, retained from the initial publication;
- replacement assets `sbom.cdx.json` (`609149226`) and `sbom.cdx.json.sha256` (`609149231`).

Upstream-alias same-version replacement:

- lightweight tag `upstream-v1.0.1`, direct runtime target `12ce97be283aebf3ded6764b7c796785b8b7781a`;
- publisher run `37178347944`, attempt 1, job `111365604011`; all 13 steps passed;
- release `402735666`, retained from the initial publication;
- replacement assets `sbom.cdx.json` (`609155886`) and `sbom.cdx.json.sha256` (`609155885`).

The native, upstream-alias and hosted SBOM/checksum files validate and are byte-identical. The replacement changed only the two v1.0.1 refs and publisher-managed release bodies/assets; the other 25 refs and 14 releases/assets remained unchanged. GitHub derived each release `created_at` value from its replacement tag target; release IDs, titles and publication state were retained. The old tag object, old runtime, initial release IDs and initial asset IDs/digests above preserve the rollback record. Runtime tag targets are immutable at `12ce97be283aebf3ded6764b7c796785b8b7781a`; this later documentation head is not a release target.

This release implements a useful R1 vertical, not complete `pi-durable` parity. Generic documents/tasks, forks, inbox modes, watches/events, extensions/hooks, durable partial output, deferred polling, compaction, subagents, remote storage, SQLite and cross-process leases remain later work.

### v1.0.1 model lookup performance replacement

The accepted and published performance runtime is `ecc458f525f0eeb5e7c2eb28544efc7a04c755e1`, with parent `e7d4ddc668992bb29714bc14d763b68a0236c533` and tree `869b7655979bfb607d93b24a3775e784a7ef1fdf`. It changes four paths: the runtime model lookup, the typed built-in lookup and their existing test files. Runtime lookup now selects the winning dynamic or baseline record before cloning it. The typed helper builds one immutable index from the generated catalogues and clones only the selected result. Existing generated records, dependencies, workflows and model-list ordering are unchanged.

The runtime preserves the previous lookup contract: the last matching dynamic model overrides the first matching baseline model; the first baseline duplicate wins without a dynamic override; missing providers and models return no value; callers receive an isolated clone; concurrent provider replacement and refresh retain valid lookup results. Typed lookup keeps chat, image and classifier identities separate and preserves the first generated record for duplicate keys.

Native Valgrind 3.27.1 `xtree-memory` evidence used identical locked baseline and candidate probes under Rust 1.96.0. Each warm estimate is the allocation-total difference between separate 100- and 10-iteration processes divided by 90; the raw records retain process setup, timing and RSS separately. For a 512-model public-registry hit, allocation fell from about 2.76 MB and 19,457 blocks per lookup to about 4.54 KB and 38 blocks. A 512-model dynamic hit fell from about 5.08 MB and 38,913 blocks to about 4.54 KB and 38 blocks. Runtime misses now allocate zero blocks. A warm typed chat hit fell from about 4.00 MB and 30,511 blocks to 2,946 bytes and 20 blocks; a warm typed miss allocates zero blocks. The typed index has a larger one-time cold cost: 7,276,278 bytes and 36,813 blocks, compared with 4,047,469 bytes and 30,608 blocks for the former single lookup. That cold cost is paid once per process and is reported separately from warm lookup measurements.

Focused tests passed **14/0** for runtime refresh/lookup and **4/0** for the typed catalogue. Full local and genuine-clone validation passed no-default **1015/0** plus one doctest and all-feature **1172/0**, with zero ignored tests. Both strict Clippy profiles, explicit Bedrock **1172/0** plus one doctest, format, build, SBOM, licence, vulnerability, exact manifests, full-record metadata, deterministic catalogue regeneration, hydration and all fail-closed mutation sentinels passed. Rust/Clippy 1.99 strict validation also passed for the exact checkpoint.

Normal push CI run `37194595514`, attempt 1, job `111413720525`, passed all 18 steps with **1172 passed, 0 failed, 0 ignored**. SHA-specific artifact `rs-ai-sbom-ecc458f525f0eeb5e7c2eb28544efc7a04c755e1` (`11300830468`) has archive SHA-256 `d5ff43ec881ef9542889eb625cb45b3550247a7dd0300179142b74d39d70e984`. Its CycloneDX 1.5 SBOM has root `rs-ai@1.0.1`, embedded revision `ecc458f525f0eeb5e7c2eb28544efc7a04c755e1`, 270 components, 271 dependency records and 23 direct root edges. The SBOM SHA-256 is `f518a5b76eeb189ab69461a2e6d170e8b5855825d339a1a01f02bcde6858880f`; the checksum-file SHA-256 is `f6c20755c9061ebc0411d779f25c853e32264760163bbaa692a7aebe9f81f35b`.

Native same-version publication:

- annotated tag `v1.0.1`, Rui-authored object `a988bdbeed57a1984ca003ce8a0bd6c993ae8bf0`, peeled runtime `ecc458f525f0eeb5e7c2eb28544efc7a04c755e1`;
- publisher run `37195282559`, attempt 1, job `111415758854`; all 13 steps passed;
- release `402735140`, retained across the same-version replacements;
- canonical assets `sbom.cdx.json` (`609618039`) and `sbom.cdx.json.sha256` (`609618038`).

Upstream-alias same-version publication:

- lightweight tag `upstream-v1.0.1`, direct runtime target `ecc458f525f0eeb5e7c2eb28544efc7a04c755e1`;
- publisher run `37195940824`, attempt 1, job `111417698682`; all 13 steps passed;
- release `402735666`, retained across the same-version replacements;
- canonical assets `sbom.cdx.json` (`609636186`) and `sbom.cdx.json.sha256` (`609636189`).

Both public releases contain exactly the canonical SBOM and checksum assets. Their sidecars validate, and their files are byte-identical to each other and to hosted artifact `11300830468`. The final inventory remains 27 tag refs and 16 releases: the accepted native tag is annotated, the alias is lightweight, both target `ecc458f525f0eeb5e7c2eb28544efc7a04c755e1`, and the other 25 refs and 14 releases/assets remain unchanged.

The prior durable publication remains a distinct rollback and history layer: native object `6d5717e64970feef066b5d2f34b1a2b6e966975a` and lightweight alias targeted `12ce97be283aebf3ded6764b7c796785b8b7781a`; retained releases `402735140` and `402735666` previously carried assets `609149226`/`609149231` and `609155886`/`609155885`, with public SBOM/checksum hashes `e2447de15d942f1be413f33dde4f728fede32474f2de993d1e473c2039636e0b` / `1af5805bae5b1f343124a3234af80964632feec6b26025d80b6d6b872dbf8b09`. The still earlier initial `73de29d...` publication layer above is also preserved rather than rewritten.

Transaction checks stopped before mutation or dispatch whenever raw state exceeded the authorised contract. Anonymous evidence downloads changed only GitHub asset `download_count` values and were recorded separately. The alias ref update changed only release `402735666`'s target-derived `created_at` and ref-update-derived `updated_at`; those exact changes were independently accepted before its single publisher dispatch. No retry, fallback, release recreation or automatic rollback occurred.

This lookup optimisation does not expand durable scope or complete `pi-durable` parity. Generic documents/tasks, forks, inbox modes, watches/events, extensions/hooks, durable partial output, deferred polling, compaction, subagents, remote storage, SQLite and cross-process leases remain later work. Runtime tags target `ecc458f525f0eeb5e7c2eb28544efc7a04c755e1`; a later documentation commit must never become a release target.

## Historical accepted release: v1.0.0

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v1.0.0`
- Upstream tag/npm gitHead: `a13d35a742c6ef8462812a28fbe1d8c8b7431c32`
- Previous accepted upstream: `v0.99.2` / `005af57d88ee23b33778f343a9595b32e67ff788`
- Accepted rs-ai runtime: `29dae5e27a90bf45da71c107102eaed61bd8418d`
- Accepted runtime tree: `bb319969d684c029f939ac056fdd4151bcbc721b`
- Audited range: `005af57d88ee23b33778f343a9595b32e67ff788..a13d35a742c6ef8462812a28fbe1d8c8b7431c32`
- Scope: `packages/ai` only; official tag and signed npm artifact.
- Scope status: **PUBLISHED NATIVE AND UPSTREAM ALIAS**. GitHub Actions run `36935351359`, job `110614195955`, passed the normal push CI gates for the exact runtime SHA. Native and upstream-alias publisher runs `36974911135` and `36975082618` completed successfully.

### v1.0.0 release inventory

The bounded range contains **8 changed paths**, `+192/-14`, and **3 changed test paths**. The final package test/support corpus contains **171 unique basenames**. `scripts/validate_v100_manifests.py --require-complete` checks the exact inventories, hashes and all dispositions in `docs/v100-171-test-crosswalk.md`. The pinned npm tarball SHA-256 is `f39b99c29b8598f175b10840e5d2a81983e7c0ce5cae4d7df83a1007447d2c2b`.

The signed schema-v6 catalog contains **1532 text/chat models across 41 chat-bearing providers and 10 APIs**, including **73 batch aliases**. It also contains **57 image models** and **15 classifier models**, for **1604 typed records across 42 provider modules**. The v0.99.2→v1.0.0 full-record deltas are chat `+5/-2/19 changed`, image `+0/-0/0 changed`, and classifier `+0/-0/1 changed`. The provider-data structure hash is `235f2f320916ab6b0d7193e0bf66ec7983e9bc05abeddd7264923fb1e7eaf76e`. Chat and classifier catalogs are regenerated from the exact tarball shards; the unchanged image catalog remains byte-identical to the accepted v0.99.2 source.

### v1.0.0 accepted runtime scope

OpenAI Responses replay resolves grammar capability and transcript tool state once for declarations, assistant calls and tool results. Custom grammar calls/results use their native wire types, preserve `call_id`, retain only compatible `ctc_` item IDs, and map missing or null grammar arguments to an empty string. Historical foreign function calls retain their accepted `fc_<shortHash>` normalisation.

The registered Anthropic OAuth adapter adds host-driven browser and copy-code login without changing the shared refresh trait. It uses the exact PKCE/state, endpoints, redirects, scopes and JSON token request shapes, and threads caller cancellation through host hooks and token requests. The shared callback server completes exchange before showing success, renders the official Pi SVG, escapes dynamic text and keeps token failures out of the public page.

The v1.0.0 classifier declaration and System One/llama.cpp wire contract were audited separately because those upstream paths are unchanged. Existing Rust types and transports already match the canonical state, question and answer shapes, `bool`↔`noul` mapping, ordered labels, score calculation, confidence and malformed-answer usage retention. Focused production tests lock those existing behaviours; no classifier runtime change was needed.

### v1.0.0 security remediation

The Bedrock dependency disables the AWS SDK's legacy default transport feature and explicitly retains `default-https-client` plus `rt-tokio`. AWS SDK versions remain unchanged. The lockfile removes only eight unreachable legacy transport packages: `h2 0.3.27`, Hyper 0.14, hyper-rustls 0.24, rustls 0.21, rustls-webpki 0.101, sct 0.7, socket2 0.5 and tokio-rustls 0.24. The resolved graph retains `h2 0.4.19`, Hyper 1.11, rustls 0.23 and rustls-webpki 0.103 through the modern AWS HTTPS client.

The four expired AWS legacy vulnerability exceptions were removed rather than renewed. `scripts/vuln_check.py` now fails closed if the forbidden legacy versions reappear or the modern HTTP/TLS families disappear. Hosted cargo-audit reported zero vulnerabilities and zero warnings; no vulnerability exceptions remain. This security slice is independent of the eight-path v1.0.0 upstream parity delta.

### v1.0.0 local and hosted acceptance

The accepted runtime passed the final local matrices with **1094/0** all-target/all-feature tests, **941/0** no-default tests and **1094/0** explicit Bedrock tests, with no ignored tests. Format, build and strict all-target/all-feature Clippy passed. A detached real-Git worktree built from the byte-identical candidate patch passed **1094/0**, strict Clippy, the security gates and the public empty-input and OAuth future-drop lifecycle probes.

The exact tarball-driven generator produced **1532** chat, **57** image and **15** classifier records. Metadata, provider/id pair equality, full-record delta mutation faults, metadata-name faults and double generation passed. Generated chat SHA-256 was `b3326c65fe95e1693ad7c8bf0b30fa59556d296c43e93221866c4ed99a9c96cf`; classifier SHA-256 was `5dcc6add6fbb1e2dc86692dd5080edc2e674b4e73cfbe076285e7e6340089485`. The unchanged committed image source retained SHA-256 `efa9537e7ac483933477b1fdfe8f54c07a135c150e6bc5a7ebf198dfe46faf1f`.

Hosted acceptance evidence:

- Runtime commit: `29dae5e27a90bf45da71c107102eaed61bd8418d`
- Runtime tree: `bb319969d684c029f939ac056fdd4151bcbc721b`
- Rollback commit: `9717d66fefed7486ecc39df9cf7e84603f1a0e75`
- GitHub Actions run: `36935351359` (normal push, attempt 1)
- Successful job: `build-test-lint` (`110614195955`), all 18 steps green
- SHA-specific artifact: `rs-ai-sbom-29dae5e27a90bf45da71c107102eaed61bd8418d` (`11197762487`)
- Artifact archive SHA-256: `b0c29466ab2a6a30b4a6619e3df26f0ea79c37fc41345ddab8a239d7e8e5e959`
- Embedded `sbom.cdx.json` SHA-256: `c49d6c2855a64bd873f2faa724b70af17febf95f923c8334a7d842c81501d6b1`
- Embedded VCS revision: `29dae5e27a90bf45da71c107102eaed61bd8418d`
- Root package: `rs-ai` version `1.0.0`, purl `pkg:cargo/rs-ai@1.0.0`
- CycloneDX version: **1.5**
- SBOM components: **270**
- SBOM dependencies: **271**
- Direct root dependency edges: **23**
- Licence review: **270** third-party packages; licence and vulnerability self-tests passed **10/10** and **8/8**
- Vulnerability scan: **0 vulnerabilities**, **0 warnings**, no expired waivers or accepted exceptions

The downloaded hosted SBOM and checksum passed strict validation and were byte-identical to the post-commit local files.

### v1.0.0 native and upstream-alias publication

The accepted runtime and documentation history are separate:

- Runtime and immutable tag target: `29dae5e27a90bf45da71c107102eaed61bd8418d`
- Runtime tree: `bb319969d684c029f939ac056fdd4151bcbc721b`
- Rollback commit: `9717d66fefed7486ecc39df9cf7e84603f1a0e75`
- Pre-publication documentation head: `d37b0499d06d15b7ced0eae8a72dcf46bfac08d5`
- Final publication receipt: the later `RELEASE.md`-only `[skip ci]` commit whose parent is `d37b0499d06d15b7ced0eae8a72dcf46bfac08d5`; its SHA is the final documentation head, never a release target

Native publication:

- Annotated tag: `v1.0.0`
- Tag object: `45eed46d6f3add945b0f3742f6603ac84d1fe40e`
- Tagger: `Rui Carmo <rui.carmo@gmail.com>`
- Tag message: `rs-ai v1.0.0`
- Peeled target: `29dae5e27a90bf45da71c107102eaed61bd8418d`
- Publisher run: `36974911135` (attempt 1), job `110736524631`; all 13 steps passed
- Release: `https://github.com/rcarmo/rs-ai/releases/tag/v1.0.0` (`401592587`)
- `sbom.cdx.json`: asset `604999940`, SHA-256 `c49d6c2855a64bd873f2faa724b70af17febf95f923c8334a7d842c81501d6b1`
- `sbom.cdx.json.sha256`: asset `604999939`, SHA-256 `ffe1ea469d5e484911155b89211f42a16fbc824a5a8850035d920703e2bd4096`

Upstream-alias publication:

- Lightweight tag: `upstream-v1.0.0`
- Direct target: `29dae5e27a90bf45da71c107102eaed61bd8418d`
- Publisher run: `36975082618` (attempt 1), job `110737045269`; all 13 steps passed
- Release: `https://github.com/rcarmo/rs-ai/releases/tag/upstream-v1.0.0` (`401593495`)
- `sbom.cdx.json`: asset `605002966`, SHA-256 `c49d6c2855a64bd873f2faa724b70af17febf95f923c8334a7d842c81501d6b1`
- `sbom.cdx.json.sha256`: asset `605002964`, SHA-256 `ffe1ea469d5e484911155b89211f42a16fbc824a5a8850035d920703e2bd4096`

Both publisher runs used workflow head `d37b0499d06d15b7ced0eae8a72dcf46bfac08d5` on `main` and the explicit runtime input `29dae5e27a90bf45da71c107102eaed61bd8418d`. Their security, licence and provenance gates passed. Both releases contain only the canonical SBOM and checksum assets. The files are byte-identical to each other and to the accepted hosted runtime evidence: CycloneDX 1.5, root `rs-ai@1.0.0`, embedded revision `29dae5e27a90bf45da71c107102eaed61bd8418d`, 270 components, 271 dependencies and 23 direct root edges. Cargo audit reported zero vulnerabilities and zero warnings; no expired waivers or accepted exceptions remain.

The v1.0.0 tags must continue to target the accepted runtime, never `d37b0499d06d15b7ced0eae8a72dcf46bfac08d5` or the final documentation receipt. The accepted v0.99.2 native and upstream refs, release IDs and asset digests remain unchanged. Native pi-durable work is a separate future feature cycle and has no source, dependency, publication or completion status in this v1.0.0 release.

## Historical accepted release: v0.99.2

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v0.99.2`
- Upstream tag/npm gitHead: `005af57d88ee23b33778f343a9595b32e67ff788`
- Previous accepted upstream: `v0.99.1` / `d86654abb8862e201933517d6f1fce9f88dd117f`
- Accepted rs-ai runtime: `255fdcccc2b392bfed8b1590b241cb6d264c4fa6`
- Accepted runtime tree: `f77669fe3f3cf32356e1fc78cb6a9f8c8980e620`
- Audited range: `d86654abb8862e201933517d6f1fce9f88dd117f..005af57d88ee23b33778f343a9595b32e67ff788`
- Scope: `packages/ai` only; official tag and signed npm artifact.
- Scope status: **ACCEPTED RUNTIME / publication blocked**. GitHub Actions run `36784618288`, job `110122915626`, passed the normal push CI gates for the exact runtime SHA. Tag, release and upstream alias creation require separate authorisation.

### v0.99.2 release inventory

The bounded range contains **15 changed paths**, `+726/-77`, and **6 changed test paths**. The final package test/support corpus contains **171 unique basenames**. `scripts/validate_v0992_manifests.py --require-complete` checks the byte-exact name-status/path inventories, derived basename inventories, hashes, and all path/test dispositions in `docs/v0992-171-test-crosswalk.md`.

The signed schema-v6 catalog contains **1529 text/chat models across 41 providers and 10 APIs**, including **75 batch aliases**. It also contains **57 image models** and **15 classifier models**, for **1601 typed records across 42 provider files**. The v0.99.1→v0.99.2 full-record deltas are chat `+6/-0/23 changed`, image `+0/-0/0 changed`, and classifier `+3/-0/0 changed`. The provider-data structure hash is `3e97a64c71ef31a515f668d9fbc653d49b3ece171d88bfd103001e963497661f`; the pinned npm tarball SHA-256 is `0b3df8791b488216f309d908789294a744bb61bbaad123d94098e56df9538d25`.

### v0.99.2 candidate runtime scope

The candidate adds Anthropic workload identity federation through deterministic Rust environment/file/token-exchange/Bearer-auth and cached credential paths. It keeps Anthropic JavaScript SDK constructor/default credential-chain mechanics as N/A and real workload credentials/network as live-only. Provider-aware Anthropic strict tool schemas retain supported constraints, fall back for rejected keywords, and fail closed when strict mode is required. Eager tool-input behavior remains an independent compatibility predicate. Z.AI CN `Prompt exceeds max length` is classified as context overflow. Invalid and non-finite `Retry-After` values fall back to exponential delay through the production HTTP retry path. The Node `./models` loader contract is N/A; Rust registry/faux access documents the analogous public surface without claiming module-loader parity.

### v0.99.2 local and hosted acceptance

The accepted runtime passed the final local matrices with **1074/0** all-target/all-feature tests, **924/0** no-default tests and **1074/0** explicit Bedrock tests. The Anthropic federation focused suite passed **9/0**. Format, strict all-target/all-feature Clippy, exact catalog/reproducibility, manifest/corruption, licence, RustSec and security gates passed. A detached real-Git worktree built from the accepted patch passed **1074/0**, strict Clippy and the security checks. The security review covered **278 third-party packages**.

Hosted acceptance evidence:

- Runtime commit: `255fdcccc2b392bfed8b1590b241cb6d264c4fa6`
- Runtime tree: `f77669fe3f3cf32356e1fc78cb6a9f8c8980e620`
- Rollback commit: `b46914a4990ecd5f445c0437c55785f86e455797`
- GitHub Actions run: `36784618288`
- Successful job: `build-test-lint` (`110122915626`)
- SHA-specific artifact: `rs-ai-sbom-255fdcccc2b392bfed8b1590b241cb6d264c4fa6` (`11128719823`)
- Artifact archive SHA-256: `abd3a4219df96eb4309d9af2de548fc789fa9e019269024b885572d541dd5752`
- Embedded `sbom.cdx.json` SHA-256: `14c28facdb8a85bbce1b97e2a7291adafd0962260b542e3d3a58c2f7b26f38f5`
- Embedded VCS revision: `255fdcccc2b392bfed8b1590b241cb6d264c4fa6`
- Root package: `rs-ai` version `0.99.2`, purl `pkg:cargo/rs-ai@0.99.2`
- CycloneDX version: **1.5**
- SBOM components: **278**
- SBOM dependencies: **279**

The downloaded hosted SBOM passed its checksum file and was byte-identical to the post-commit local SBOM. Publication is still blocked. Any later v0.99.2 tag must target runtime commit `255fdcccc2b392bfed8b1590b241cb6d264c4fa6`, not the later documentation head.

## Historical accepted release: v0.99.1

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v0.99.1`
- Upstream tag/npm gitHead: `d86654abb8862e201933517d6f1fce9f88dd117f`
- Previous accepted upstream: `v0.87.1` / `f07218c4d4bbc12bef056a7058c3dd49dfe41abe`
- Accepted rs-ai runtime: `32b07c7fb3ab336f6a9709bf5eb14286af73958c`
- Audited range: `f07218c4d4bbc12bef056a7058c3dd49dfe41abe..d86654abb8862e201933517d6f1fce9f88dd117f`
- Scope: `packages/ai` only; official tag and signed npm artifact.
- Scope status: **ACCEPTED**. GitHub Actions run `36634981815`, job `109633298070`, passed all 18 steps with 1055 tests plus format, build, strict Clippy, SBOM, licence and vulnerability gates.

### v0.99.1 release inventory

The bounded range contains **169 changed paths**, `+7001/-2913`, and **58 changed test paths**. The final upstream corpus contains **160 test basenames**. `scripts/validate_v0991_manifests.py --require-complete` checks the exact inventories and hashes recorded under `docs/manifests/`; `docs/v0991-160-test-crosswalk.md` records all path and test dispositions with no pending rows.

The signed schema-v6 catalog contains **1523 text/chat models across 41 providers and 10 APIs**, including **75 batch aliases**. It also contains **57 image models** and **12 classifier models**, for **1592 typed records**. The full-record deltas are text `1495→1523`, `+59/-31/110 changed`; images `55→57`, `+2/-0/54 changed`; classifiers `0→12`, `+12/-0/0 changed`. The pinned npm tarball SHA-256 is `f9f44692157d0bf5679c4a17304a310028231d7daaeaaea3b73252f4b7a264d3`.

### v0.99.1 runtime scope

The accepted runtime adds a unified typed model catalog and classifier operations for TypeSafe System One, Cloudflare Workers AI, and llama.cpp. It adds OpenAI ChatGPT OAuth, provider-native stream-event hooks, requested thinking-level metadata, nested tool-call metadata, and fail-closed OpenAI Responses terminal handling. Codex WebSocket and Bedrock provider events reach callbacks before normalisation; callback failures terminate with `StopReason::Error`. Deterministic tests cover callback ordering and failure, OAuth callback/manual/cancel/timeout/IPv6 paths, catalog operations, classifier HTTP production paths, provider reasoning and sampling changes, and incomplete tool-call rejection.

### v0.99.1 CI and SBOM evidence

- Runtime commit: `32b07c7fb3ab336f6a9709bf5eb14286af73958c`
- GitHub Actions run: `36634981815`
- Successful job: `build-test-lint` (`109633298070`), all 18 steps green
- SHA-specific artifact: `rs-ai-sbom-32b07c7fb3ab336f6a9709bf5eb14286af73958c` (`11064098033`)
- Artifact ZIP SHA-256: `8019316813ac7e0a350688142a4b573ccdecd768756428b191212197082bf354`
- Embedded `sbom.cdx.json` SHA-256: `986785a7d292e348234f81b88f59145c2e1ec58806e83140714c120d22fe734e`
- Embedded VCS revision: `32b07c7fb3ab336f6a9709bf5eb14286af73958c`
- CycloneDX version: **1.5**
- SBOM components: **278**
- SBOM dependencies: **279**

The guarded publisher uses tag `upstream-v0.99.1`, checks out the accepted runtime SHA, reruns the security gates, verifies the embedded revision, and publishes `sbom.cdx.json` plus `sbom.cdx.json.sha256`. The v0.87.1, v0.87.0, and v0.85.1 sections and assets remain historical records.

## Historical accepted release: v0.87.1

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v0.87.1`
- Upstream tag/npm gitHead: `f07218c4d4bbc12bef056a7058c3dd49dfe41abe`
- Previous accepted upstream: `v0.87.0` / `16787ad5b2dc748047f314ca1bfe7708f30f54f3`
- Accepted rs-ai runtime: `f7c257bf240ae979cd5962513e1e5d167dd9fe2f`
- Audited range: `16787ad5b2dc748047f314ca1bfe7708f30f54f3..f07218c4d4bbc12bef056a7058c3dd49dfe41abe`
- Scope: `packages/ai` only; official tag and signed npm artifact.
- Scope status: **ACCEPTED**. GitHub Actions run `35793644589`, job `106967752166`, passed all 18 steps with 1027 tests plus format, build, strict Clippy, SBOM, licence and vulnerability gates.

### v0.87.1 release inventory

The bounded range contains **16 changed paths**, `+386/-78`, and **9 changed test paths**. The final upstream corpus remains **150 test basenames**. `scripts/validate_v0871_manifests.py` checks the exact inventories and hashes recorded under `docs/manifests/`; `docs/v0871-150-test-crosswalk.md` records all path and test dispositions.

The signed catalog contains **1495 text/chat models across 41 providers and 10 APIs**, including **71 batch aliases**. The image catalog contains **55 models across one provider and one image API**. The full-record deltas are text `+62/-12/35 changed` and images `+1/-0/0 changed`. The pinned npm tarball SHA-256 is `35b4432f27cc2665f86beebb9af6a39b1251970883c3044bd8be4f4e8c731ca0`.

### v0.87.1 runtime scope

The accepted runtime omits exactly-empty text parts from OpenAI-compatible multimodal user messages while preserving whitespace, updates the Claude Code OAuth user agent to `claude-cli/2.1.280`, and regenerates exact catalog metadata for Grok 4.7, Claude Opus 5.5, GPT-6 Sol/Luna, GitHub Copilot aliases and OpenRouter Ming Image. Deterministic production-path tests cover Grok 4.7 Responses routing, xhigh effort and encrypted reasoning, Opus 5.5 effort/context/pricing, Sol/Luna cache/default reasoning/Codex max effort, and Copilot aliases.

### v0.87.1 CI and SBOM evidence

- Runtime commit: `f7c257bf240ae979cd5962513e1e5d167dd9fe2f`
- GitHub Actions run: `35793644589`
- Successful job: `build-test-lint` (`106967752166`), all 18 steps green
- SHA-specific artifact: `rs-ai-sbom-f7c257bf240ae979cd5962513e1e5d167dd9fe2f` (`10723765029`)
- Artifact ZIP SHA-256: `7a788b0a4eeebe46ef4fea94380a3298bf90625f3ed54b864d95825a40997447`
- Embedded `sbom.cdx.json` SHA-256: `41b09272fbb1d30046d384ed8b14f73f6705f36f27d0f015a5672cbe7c592dc6`
- Embedded VCS revision: `f7c257bf240ae979cd5962513e1e5d167dd9fe2f`
- CycloneDX version: **1.5**
- SBOM components: **278**

The guarded publisher uses tag `upstream-v0.87.1`, checks out the accepted runtime SHA, reruns the security gates, verifies the embedded revision, and publishes `sbom.cdx.json` plus `sbom.cdx.json.sha256`. The v0.87.0 and v0.85.1 sections and assets remain historical records.

## Historical accepted release: v0.87.0

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v0.87.0`
- Upstream tag/commit: `16787ad5b2dc748047f314ca1bfe7708f30f54f3`
- Previous accepted upstream: `v0.85.1` / `d981de1229ef899957bbe968bc8dcda02a21f477`
- Accepted rs-ai runtime: `6096fe39de3128b8371718ceef5d9ce2c8e56705`
- Audited range: `d981de1229ef899957bbe968bc8dcda02a21f477..16787ad5b2dc748047f314ca1bfe7708f30f54f3`
- Scope: `packages/ai` only; official tag and signed npm artifact.
- Scope status: **ACCEPTED**. The runtime passed GitHub Actions run `35660592647`, job `106534622292`, with 1015 tests plus format, build, strict Clippy, SBOM, licence and vulnerability gates.

### v0.87.0 release inventory

The bounded range contains **127 changed paths** and **82 changed test paths**. The final upstream corpus contains **150 test basenames**. `scripts/validate_v0870_manifests.py` checks the exact inventories and hashes recorded under `docs/manifests/`; `docs/v0870-150-test-crosswalk.md` records their accepted Rust dispositions.

The signed catalog contains **1445 text/chat models across 41 providers and 10 APIs**, including **74 batch aliases**. The image catalog contains **54 models across one provider and one image API**. The pinned npm tarball SHA-256 is `f2adf9de809d035f76f8dadf3d148720ebeef4606a848ab36ee834d895ae812f`.

### v0.87.0 runtime scope

The accepted runtime adds transcript normalization and ordered system-message replay, provider-native and collapsed tool changes, terminal/retry handling for high-demand and HTTP 520 failures, the OpenCode session header, Meta device OAuth and API-key minting, Radius baseline/dynamic catalog overlay, pi-messages transcript transport, and exact model image-limit metadata. Provider payload tests cover Anthropic native/fallback tool changes, OpenAI Responses additions/fallback, Codex instructions, and legacy transcript normalization.

### v0.87.0 CI and SBOM evidence

- Runtime commit: `6096fe39de3128b8371718ceef5d9ce2c8e56705`
- GitHub Actions run: `35660592647`
- Successful job: `build-test-lint` (`106534622292`), all 18 steps green
- SHA-specific artifact: `rs-ai-sbom-6096fe39de3128b8371718ceef5d9ce2c8e56705` (`10667527616`)
- Artifact ZIP SHA-256: `739c973b3910716228be42f5caebd8515346bc973b0ffdf8f0beaf0804a1de1f`
- Embedded `sbom.cdx.json` SHA-256: `91b155954d3aa86776b410f8649ea7e02cd52ba0b0abb57288c581fee394ccf4`
- Embedded VCS revision: `6096fe39de3128b8371718ceef5d9ce2c8e56705`
- SBOM components: **278**

The guarded publisher uses tag `upstream-v0.87.0`, checks out the accepted runtime SHA, reruns the security gates, verifies the embedded revision, and publishes `sbom.cdx.json` plus `sbom.cdx.json.sha256`. The existing v0.85.1 section and assets remain historical records.

## Historical accepted release: v0.85.1

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v0.85.1`
- Upstream tag/commit: `d981de1229ef899957bbe968bc8dcda02a21f477`
- Previous accepted upstream: `v0.85.0` / `107d79f11072bbc8a3a757ed7fd69596bee7d68c`
- Accepted rs-ai baseline: `587d7440cb4b67feeda82f81ac95fa1055ff4821`
- Audited range: `107d79f11072bbc8a3a757ed7fd69596bee7d68c..d981de1229ef899957bbe968bc8dcda02a21f477`
- Scope: `packages/ai` only; official tag/npm artifact only, no unreleased origin/main changes.
- Scope status: **ACCEPTED / completed for bounded deterministic v0.85.1 release parity**. Runtime candidate `e74bfc2c3dc1e12c8b4cedf8e2f8e26c13f299de` passed hosted CI; README/catalog prose is updated in a post-acceptance docs-only commit; durable `upstream-v0.85.1` release assets are handled by the guarded publisher workflow, whose release title/notes now derive strictly from `release_tag = upstream-vX.Y.Z`.

### v0.85.1 exact upstream path disposition

The official range changes **9** `packages/ai` paths, all modified, with total delta `+128/-23`: 4 source/scripts paths, 2 generated/package metadata paths, and 3 tests. The complete upstream test corpus remains **142** `packages/ai/test/*.test.ts` files, recorded in `docs/v0851-142-test-crosswalk.md`.

Executable validator: `python3 scripts/validate_v0851_manifests.py` asserts this exact 9-path set, the 142 unique crosswalk rows, and the committed exact-content inventory hashes (`ee26f669d92dc77b265731165a2ff69ccb67defba92517cbbd5f97a186e187d2` for changed paths; `56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065` for the test corpus). Changed test-path hash: `f7e274bf229c90fc22ba22384c5b89f71a5c6801f77067d099525a9cdc537610`.

| Status | Upstream path | rs-ai disposition |
|---|---|---|
| M | `packages/ai/CHANGELOG.md` | DOCUMENTED / metadata-only |
| M | `packages/ai/package.json` | DOCUMENTED / metadata-only |
| M | `packages/ai/scripts/generate-models.ts` | ADAPTED via extractor/generator and regenerated catalog |
| M | `packages/ai/src/api/openai-responses.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/image-models.generated.ts` | ADAPTED by regenerated image registry + metadata verifier |
| M | `packages/ai/src/types.ts` | ADAPTED in Rust types/options/compat |
| M | `packages/ai/test/cache-retention.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/max-thinking.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/supports-xhigh.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |

### v0.85.1 implementation summary

- Regenerated the text catalog from official npm `dist/providers/data` shards: **1354 text provider/id pairs across 39 providers and 9 APIs**; official OpenRouter batch aliases are now **68**. Full-record text delta from v0.85.0: `1336→1354`, `+20/-2/18 changed`.
- Regenerated the image catalog from package `IMAGE_MODELS`: **52 image provider/id pairs**, adding `microsoft/mai-image-2.6` and `microsoft/mai-image-2.6-flash`. Full-record image delta from v0.85.0: `50→52`, `+2/-0/0 changed`.
- Updated release metadata verifier defaults to `@earendil-works/pi-ai@0.85.1` and pinned npm tarball SHA-256 `af7d11986179445ce6fe88b37d57de22f823c0ffd3a65cae31c555b7f5e99253`; added full-record v0.85.0→v0.85.1 delta verification (`text +20/-2/18 changed`, `image +2/-0/0`) with mutation fault coverage.
- Preserved the exact v0.85.1 provider manifest shape: `schemaVersion=3`, `generatedAt=2026-09-05T11:58:56.761Z`, `structureHash=ff87cfcb3c1decb7ceeb4a5d71282696e108d093b7098f372d6f8a442dfed40d`, `manifestSha256=30e7f58cc33d5901dbcb64f0e00d0133620005737ffb73553b358ca23572bac8`.
- Made generated catalog headers deterministic from the release provider manifest `generatedAt` value and added `scripts/verify_generated_reproducibility.py` to prove two independent generations are byte-for-byte identical.
- Added OpenAI Responses prompt-cache serialization parity: explicit-cache models emit `prompt_cache_options.mode = "explicit"` for `cacheRetention: "none"`, `prompt_cache_options.ttl = "30m"` for supported long retention, and never emit legacy `prompt_cache_retention` concurrently; older models retain `prompt_cache_retention = "24h"`.
- Added GPT-6 Astra catalog/compat/thinking coverage across OpenAI Responses, Azure OpenAI Responses, OpenAI Codex, OpenCode, GitHub Copilot, OpenRouter, and Vercel AI Gateway aliases, including 272000/128000 OpenAI/Codex windows, text+image input, cost/tier metadata, tool-search/additional-tool flags, explicit prompt cache mode, and xhigh/max thinking maps.

Named Rust evidence added/updated for v0.85.1:

- `src/tests/providers/openai/openai_responses_prompt_cache_test.rs`
- `src/tests/release/v0851_release_test.rs`
- `src/tests/release/release_metadata_verification_test.rs::{generated_catalogs_are_byte_for_byte_reproducible,v0851_manifest_validator_confirms_changed_paths_and_crosswalk_rows,v0851_manifest_validator_detects_changed_path_inventory_corruption,v0851_manifest_validator_detects_test_corpus_inventory_corruption,v0851_baseline_delta_validator_confirms_full_record_counts,v0851_baseline_delta_validator_detects_record_mutation}`

### v0.85.1 release-pinned artifact evidence

- upstream tag worktree: `/workspace/tmp/pi-mono-audit` at `d981de1229ef899957bbe968bc8dcda02a21f477`
- unpacked npm package: `/workspace/tmp/pi-ai-0851/package`
- npm tarball: `/workspace/tmp/pi-ai-0851/earendil-works-pi-ai-0.85.1.tgz`
- npm published: `2026-09-05T12:05:47.996Z`
- npm tarball SHA-256: `af7d11986179445ce6fe88b37d57de22f823c0ffd3a65cae31c555b7f5e99253`
- npm tarball SHA-512: `f958152090e40ced9e7d824a104aaf3d31f8ce69c8697740a6919b3bebca140f6acb93dd8458807a7b8502453ea220927ce0b874c1c3cab8dd41e2f86680b909`
- changed-path manifest SHA-256: `ee26f669d92dc77b265731165a2ff69ccb67defba92517cbbd5f97a186e187d2`
- changed-test manifest SHA-256: `f7e274bf229c90fc22ba22384c5b89f71a5c6801f77067d099525a9cdc537610`
- test-corpus manifest SHA-256: `56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065`
- extracted release JSON: `/workspace/tmp/pi-v0851-json/models.json`

Commands/results run so far:

```bash
sha256sum /workspace/tmp/pi-ai-0851/earendil-works-pi-ai-0.85.1.tgz /workspace/tmp/pi-ai-0851/changed-paths.txt
sha512sum /workspace/tmp/pi-ai-0851/earendil-works-pi-ai-0.85.1.tgz
git diff --stat v0.85.0 v0.85.1 -- packages/ai
python3 scripts/validate_release_model_data.py /workspace/tmp/pi-ai-0851/package/dist/providers/data
python3 scripts/extract_release_model_shards.py /workspace/tmp/pi-ai-0851/package /workspace/tmp/pi-v0851-json --tag-worktree /workspace/tmp/pi-mono-audit --tag-sha d981de1229ef899957bbe968bc8dcda02a21f477
python3 scripts/generate_models.py /workspace/tmp/pi-v0851-json/models.json
python3 scripts/generate_image_models.py /workspace/tmp/pi-v0851-json/image-models.json
cargo test openai_responses_prompt_cache_test --all-features -- --nocapture
cargo test v0851_release_test --all-features -- --nocapture
cargo test simple_options_test --all-features -- --nocapture
python3 scripts/verify_release_model_metadata.py
python3 scripts/verify_generated_reproducibility.py
```

Final local results before candidate push: `cargo fmt --all -- --check` passed; `cargo build --all-targets` passed; focused v0.85.1 cache/max/xhigh tests passed (`openai_responses_prompt_cache_test`: 7 passed; `v0851_release_test`: 5 passed; `max_thinking_test`: 4 passed; `supports_xhigh_test`: 17 passed; `simple_options_test`: 18 passed); full `cargo test --all-targets --all-features` passed three consecutive final runs plus a fresh summary run (`993 passed`, `0 failed`, `0 ignored`); strict `cargo clippy --all-targets --all-features -- -D warnings` passed; `cargo check --no-default-features` and `cargo check --no-default-features --features bedrock` passed; `cargo test --no-default-features` passed (`853 passed`, doctest `1 passed`); `cargo test --no-default-features --features bedrock` passed (`993 passed`, doctest `1 passed`); provider data validator passed (`models=1354 providers=39 structureHash=ff87cfcb3c1decb7ceeb4a5d71282696e108d093b7098f372d6f8a442dfed40d`); metadata verifier passed (`text=1354 providers=39 apis=9 batchAliases=68 image=52`); generated catalog reproducibility passed byte-for-byte (`src/models_generated.rs` SHA-256 `50c20966d68d24f9f6442ec1257db084470e1a9ceb8b0756526c255f1a56ef6d`, `src/images/models_generated.rs` SHA-256 `36ac771fe36f2da472a4c8a879b881f2d6f21d7a2049abd84828512230937f23`); v0.85.0 and v0.85.1 manifest validators passed clean; v0.85.1 baseline delta verifier passed (`text=+20/-2/18 changed image=+2/-0/0 changed`); v0.85.0 historical manifest corruption modes and v0.85.1 changed-path/test-corpus corruption modes failed closed; text/image metadata fault gates failed closed; v0.85.1 baseline-record mutation failed closed; provider/id comparator passed with text `upstream=1354 local=1354 missing=0 extra=0` and image `upstream=52 local=52 missing=0 extra=0`; `make security-check` passed, generating/validating local SBOM with **278** dependency components and local pre-commit checksum `79a83032828328745a28fd37eb71c39aeeccf99dc728a715c301367f1a45ad46`; license self-tests/review and RustSec wrapper/cargo-audit passed with existing temporary advisory exceptions after updating `Cargo.lock` from `rustls 0.23.43` to `rustls 0.23.45` for `RUSTSEC-2026-0285`; `git diff --check` passed. Hosted CI/SHA-specific SBOM evidence: replacement runtime candidate `e74bfc2c3dc1e12c8b4cedf8e2f8e26c13f299de` passed GitHub Actions run `35156144749`, job `build-test-lint` (`104995914909`). Uploaded artifact `rs-ai-sbom-e74bfc2c3dc1e12c8b4cedf8e2f8e26c13f299de` (`10471770993`, expiry `2026-10-16T22:14:12Z`) has archive SHA-256 `89dcab0e218f52ce9ca950e6ade7e424dd9216935f93e221e9fd5028516e5a7e`, inner `sbom.cdx.json` SHA-256 `87cf86e01efc33eb9242da6d54eec4f37354e26ac5bcfa07d398b0f65d41082a`, checksum-file SHA-256 `75869fcfcbe3b1ed5dfcc7d7d818d4df80287c79c5bceebef3e3bf8d8a66028e`, checksum line `87cf86e01efc33eb9242da6d54eec4f37354e26ac5bcfa07d398b0f65d41082a  sbom.cdx.json`, embedded revision `e74bfc2c3dc1e12c8b4cedf8e2f8e26c13f299de`, and **278** components. Superseded candidate `1b624cfce3dc2c3b9ed534b8c73305a3589896af` passed CI build/test/SBOM/license but failed hosted RustSec on newly reported `RUSTSEC-2026-0285` for `rustls 0.23.43`; it is retained only as failed/superseded evidence.

## Historical accepted release: v0.85.0

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v0.85.0`
- Upstream tag/commit: `107d79f11072bbc8a3a757ed7fd69596bee7d68c`
- Previous accepted upstream: `v0.84.4` / `b79e4cc834970cca69daebffab7df1da7d1e52c4`
- Accepted rs-ai baseline: `180d9c216f8ba52b6d0812355fb64f62fed2ea5d`
- Audited range: `b79e4cc834970cca69daebffab7df1da7d1e52c4..107d79f11072bbc8a3a757ed7fd69596bee7d68c`
- Scope: `packages/ai` only; official tag/npm artifact only, no newer-main chase.
- Scope status: **ACCEPTED**. Runtime `0eb50d428d75a0281231fcd294d768c3db9cd17c` passed hosted CI and auditor review; final README/docs head `e355b256bcc68c4b76a3ad3558e83d6d725b5ee3` was README-only `[skip ci]`.

### v0.85.0 exact upstream path disposition

The official range changes **51** `packages/ai` paths: 19 source/scripts paths (16 modified, 2 added, 1 deleted), 29 tests (22 modified, 6 added, 1 deleted), and 3 package/docs paths. Final corpus: **142** upstream `packages/ai/test/*.test.ts` files, recorded in `docs/v0850-142-test-crosswalk.md`.

Executable validator: `python3 scripts/validate_v0850_manifests.py` asserts this exact 51-path set, the 142 unique crosswalk rows, and the committed exact-content inventory hashes (`db461a56838926cf60d4ae0196ed98fcc215616dacff013ad8c235bb8ad9b83f` for changed paths; `56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065` for the test corpus).

| Status | Upstream path | rs-ai disposition |
|---|---|---|
| M | `packages/ai/CHANGELOG.md` | DOCUMENTED / metadata-only |
| M | `packages/ai/README.md` | DOCUMENTED / metadata-only |
| M | `packages/ai/package.json` | DOCUMENTED / metadata-only |
| M | `packages/ai/scripts/generate-models.ts` | ADAPTED via extractor/generator and regenerated catalog |
| M | `packages/ai/src/api/anthropic-messages.ts` | ADAPTED in Rust provider/runtime code |
| A | `packages/ai/src/api/cloudflare-ai-binding.ts` | N/A Workers binding object; Rust HTTP/gateway semantics documented |
| D | `packages/ai/src/api/cloudflare-gateway-binding.ts` | N/A deleted upstream Workers binding object |
| M | `packages/ai/src/api/openai-codex-responses.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-completions.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-responses-shared.ts` | ADAPTED / COVERED |
| M | `packages/ai/src/api/openai-responses.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/pi-messages.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/index.ts` | N/A TypeScript export surface; Rust exposes modules directly |
| M | `packages/ai/src/models.ts` | ADAPTED / COVERED by runtime registry tests |
| M | `packages/ai/src/providers/cloudflare-ai-gateway.ts` | ADAPTED / COVERED |
| M | `packages/ai/src/providers/faux.ts` | ADAPTED by origin faux context-capture commit and tests |
| M | `packages/ai/src/providers/openrouter.ts` | ADAPTED by regenerated catalog metadata |
| M | `packages/ai/src/types.ts` | ADAPTED in Rust types/options/compat |
| A | `packages/ai/src/utils/assistant-message-frame.ts` | ADAPTED as public Rust `assistant_message_frame` encoder/reducer |
| M | `packages/ai/src/utils/node-http-proxy.ts` | ADAPTED in Rust HTTP proxy utility |
| M | `packages/ai/src/utils/retry.ts` | COVERED by existing retry tests |
| M | `packages/ai/src/utils/uuid.ts` | ADAPTED in Rust uuid utility |
| M | `packages/ai/test/anthropic-auth-token.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/anthropic-cache-write-1h-cost.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| A | `packages/ai/test/anthropic-mid-conversation-effort.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/anthropic-sse-parsing.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| A | `packages/ai/test/anthropic-thinking-binding-e2e.test.ts` | LIVE UNEXECUTED / deterministic binding payload covered |
| A | `packages/ai/test/assistant-message-frame.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/baseten-models.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| A | `packages/ai/test/cloudflare-ai-binding.test.ts` | N/A Workers binding object; Rust HTTP/gateway semantics documented |
| D | `packages/ai/test/cloudflare-gateway-binding.test.ts` | N/A deleted upstream Workers binding test |
| M | `packages/ai/test/constrained-sampling.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/generate-models-strict.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/github-copilot-anthropic.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/github-copilot-oauth.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/node-http-proxy.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/openai-codex-stream.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/openai-completions-cache-control-format.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/openai-completions-thinking-as-text.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/openai-completions-tool-choice.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/openai-completions-tool-result-images.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| A | `packages/ai/test/openai-completions-vllm-priority.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/openai-responses-compat.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/openai-responses-namespace.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/openrouter-cache-control-models.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/pi-messages.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| A | `packages/ai/test/pre-generation-error.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/qwen-token-plan-models.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/tool-call-id-normalization.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/uuid.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |
| M | `packages/ai/test/xai-responses.test.ts` | ADAPTED / COVERED (see 142-test crosswalk) |

### v0.85.0 implementation summary

- Regenerated the text catalog from official npm `dist/providers/data` shards: **1336 text provider/id pairs across 39 providers and 9 APIs**; official OpenRouter batch aliases are now **66**. Full-record text delta from v0.84.4: `1290→1336`, `+72/-26/79 changed`.
- Verified image catalog remains **50 image provider/id pairs**. Full-record image delta from v0.84.4: `50→50`, `+0/-0/0`.
- Updated release metadata verifier defaults to `@earendil-works/pi-ai@0.85.0` and pinned npm tarball SHA-256 `46188bdacb555a07466a0111f3963f20932a16199e4d6cfb8d44a7fe5fc6e342`; added full-record v0.84.4→v0.85.0 delta verification (`text +72/-26/79 changed`, `image +0/-0/0`) with mutation fault coverage.
- Preserved the exact v0.85.0 provider manifest shape: `schemaVersion=3`, `structureHash=a71a055905e4b12c9bb41fa6c2bf90fb2944ca76843bdfd3831377caba1be189`, `manifestSha256=61b723edeaf75dd3d55908da3530b8bf013ecffa4dc7734f4220ef0bc64442c7`.
- Added `Message.providerThinkingLevel` serialization and Anthropic managed mid-conversation effort support: beta headers, adaptive `block_binding`, high output-config marker, and stream metadata preservation.
- Added public `AssistantMessageFrame`, `AssistantMessageFrameEncoder`, and `reduce_assistant_message_frames` support for provider-neutral assistant progress persistence/replay, with deterministic coverage for live-partial offset/prefix reconciliation, authoritative end metadata/arguments, tool JSON checkpoint/resume, interleaving, clone/purity, terminal omission/state, pre-generation error grammar, strict kind/order/index/end rejection, exact golden frame JSON, assistant-specific start-frame wire without ToolResult-only `isError`, required start-frame fields (`content`, `timestamp`, `api`, `provider`, `model`, `usage`, `stopReason`), rejection of non-assistant/non-empty/non-pending starts, presence-aware `thinking.redacted` omission vs explicit `false`, no-null optional frame metadata, camelCase embedded `ContentBlock` wire fields (`textSignature`, `thinkingSignature`, `thoughtSignature`, `mimeType`), legacy snake_case decode aliases, and decode-time malformed/unknown nested shape rejection.
- Added OpenAI-compatible `compat.vllmPriority` -> top-level `priority`, OpenAI Responses `supportsMaxOutputTokens=false` omission and successful/length/toolUse terminal stale-error cleanup, explicit UUIDv7 timestamp support, NO_PROXY suffix/port coverage, pi-messages `providerThinkingLevel`, and Codex terminal SSE without trailing blank coverage.
- Reflected strict generated catalog deltas: `qwen3.8-flash` Individual allowlist plus new audited OpenRouter batch aliases `anthropic/claude-fable-5.1`, `google/gemini-3.8-flash`, and `x-ai/grok-4.3`.
- Assessed Cloudflare Workers AI binding replacement as N/A for Rust Workers binding-object semantics; Rust HTTP/gateway/provider catalog behavior remains covered.

Named Rust evidence added/updated for v0.85.0:

- `src/tests/release/v0850_release_test.rs::release_pinned_catalog_counts_match_v0850`
- `src/tests/release/v0850_release_test.rs::openai_completions_vllm_priority_serializes_top_level_priority`
- `src/tests/release/v0850_release_test.rs::openai_responses_max_output_tokens_respects_compat_flag`
- `src/tests/release/v0850_release_test.rs::message_serializes_provider_thinking_level_camel_case`
- `src/tests/release/v0850_release_test.rs::uuidv7_accepts_explicit_timestamp_and_rejects_overflow`
- `src/tests/release/v0850_release_test.rs::no_proxy_matches_uppercase_suffix_and_port_rules`
- `src/tests/providers/anthropic/anthropic_mid_conversation_effort_test.rs`
- `src/tests/core/pi_messages_test.rs::streams_text_tool_calls_payload_and_terminal_message`
- `src/tests/core/assistant_message_frame_test.rs`
- `src/tests/providers/openai/openai_responses_terminal_event_test.rs::{terminal_success_and_length_clear_stale_incomplete_error_messages,terminal_tool_use_clears_stale_incomplete_error_message,terminal_incomplete_error_replaces_stale_error_with_provider_reason}`
- `src/tests/providers/other/pre_generation_error_test.rs::missing_auth_surfaces_first_error_from_concrete_provider_streams`
- `src/tests/providers/codex/openai_codex_stream_test.rs::processes_terminal_sse_event_without_trailing_blank_line`
- `src/tests/release/release_metadata_verification_test.rs::{v0850_manifest_validator_confirms_changed_paths_and_crosswalk_rows,v0850_manifest_validator_detects_changed_path_inventory_corruption,v0850_manifest_validator_detects_test_corpus_inventory_corruption,v0850_baseline_delta_validator_confirms_full_record_counts,v0850_baseline_delta_validator_detects_record_mutation}`

### v0.85.0 release-pinned artifact evidence

- upstream tag worktree: `/workspace/tmp/pi-mono-audit` at `107d79f11072bbc8a3a757ed7fd69596bee7d68c`
- unpacked npm package: `/workspace/tmp/pi-ai-0850/package`
- npm tarball SHA-256: `46188bdacb555a07466a0111f3963f20932a16199e4d6cfb8d44a7fe5fc6e342`
- npm tarball SHA-512: `09b79e647dcd1dabfb46cd7cdad62ad1ea020167c377532f3805cace89c8178b8ddee3bdf4407c893d477f21a87c998f9007fc31e23038142daaa774ce0acf58`
- changed-path manifest SHA-256: `db461a56838926cf60d4ae0196ed98fcc215616dacff013ad8c235bb8ad9b83f`
- test-corpus manifest SHA-256: `56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065`
- extracted release JSON: `/workspace/tmp/pi-v0850-json/models.json`

Commands/results run so far:

```bash
sha256sum /workspace/tmp/pi-ai-0850/pi-ai-0.85.0.tgz /workspace/tmp/pi-ai-0850/changed-paths.txt /workspace/tmp/pi-ai-0850/test-corpus-142.txt
python3 scripts/validate_release_model_data.py /workspace/tmp/pi-ai-0850/package/dist/providers/data
python3 scripts/extract_release_model_shards.py /workspace/tmp/pi-ai-0850/package /workspace/tmp/pi-v0850-json --tag-worktree /workspace/tmp/pi-mono-audit --tag-sha 107d79f11072bbc8a3a757ed7fd69596bee7d68c
python3 scripts/generate_models.py /workspace/tmp/pi-v0850-json/models.json
python3 scripts/generate_image_models.py /workspace/tmp/pi-v0850-json/image-models.json
python3 scripts/verify_release_model_metadata.py
python3 scripts/validate_v0850_manifests.py
python3 scripts/validate_v0850_manifests.py --fault v0850-changed-paths
python3 scripts/validate_v0850_manifests.py --fault v0850-test-corpus-142
python3 scripts/verify_v0850_baseline_delta.py
python3 scripts/verify_v0850_baseline_delta.py --fault baseline-record
cargo test v0850_release_test -- --nocapture
cargo test anthropic_mid_conversation_effort_test -- --nocapture
cargo test assistant_message_frame_test -- --nocapture
cargo test openai_responses_terminal_event_test -- --nocapture
cargo test pre_generation_error_test -- --nocapture
cargo test pi_messages_test -- --nocapture
cargo test openai_codex_stream_test::tests::processes_terminal_sse_event_without_trailing_blank_line -- --nocapture
```

Strict frame-decode corrective local results: `cargo fmt --all -- --check` passed; `cargo build` passed; focused assistant-message-frame strict decode tests passed (`20 passed`); focused Responses terminal/serialized-error tests passed (`8 passed`); full `cargo test --all-targets --all-features` passed three consecutive times (`975 passed`, `0 failed`, `0 ignored` each); strict `cargo clippy --all-targets --all-features -- -D warnings` passed; `cargo check --no-default-features` and `cargo check --no-default-features --features bedrock` passed; `cargo test --no-default-features` passed (`846 passed`, doctest `1 passed`); `cargo test --no-default-features --features bedrock` passed (`975 passed`, doctest `1 passed`); metadata verifier passed (`text=1336 providers=39 apis=9 batchAliases=66 image=50`); metadata fault gates failed closed for `text-name` and `image-name`; provider/id comparator passed with text `upstream=1336 local=1336 missing=0 extra=0` and image `upstream=50 local=50 missing=0 extra=0`; release metadata suite passed (`11 passed`); manifest validator passed with `changedSha256=db461a56838926cf60d4ae0196ed98fcc215616dacff013ad8c235bb8ad9b83f` and `testCorpusSha256=56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065`; manifest fault gates failed closed for both inventories; baseline delta verifier passed (`text=+72/-26/79 changed image=+0/-0/0 changed`) and `--fault baseline-record` failed closed; `make check` passed, generating/validating local SBOM `artifacts/sbom.cdx.json` with **278** dependency components and local checksum `1ac8e9317c86c5d1a10f67020c04bdadf81f5ecc0a347b5574bafe8926f7a6aa`; license self-tests/review and RustSec wrapper/cargo-audit passed after updating `Cargo.lock` from `rustls 0.23.43` to `rustls 0.23.45` for `RUSTSEC-2026-0285`; `git diff --check` passed. Strict frame-decode corrective runtime SHA `0eb50d428d75a0281231fcd294d768c3db9cd17c` passed hosted GitHub Actions run `33896845145`, job `build-test-lint` (`101101379038`); artifact `rs-ai-sbom-0eb50d428d75a0281231fcd294d768c3db9cd17c` (`9946095734`) has GitHub archive digest `sha256:5e5a9c9fd928da01afb4e3d0140bbfb4296a19ea6d332d892b46ea7c7d549ff3`, downloaded archive SHA-256 `5e5a9c9fd928da01afb4e3d0140bbfb4296a19ea6d332d892b46ea7c7d549ff3`, inner CI `sbom.cdx.json` SHA-256 `24256dacc79097c90f399632852967c868371d8f64c85a5c7dea91ca3bd17284`, checksum-file SHA-256 `335a20e305c0edc6dd39e4cfd8297f8333bd36642c89b2a145a4a213b4692d19`, embedded revision `0eb50d428d75a0281231fcd294d768c3db9cd17c`, **278** components, and expiry `2026-10-04T16:46:02Z`. This evidence is recorded in a follow-up docs-only SHA; superseded exact frame-wire candidate `0c53d6eb06eebb61581ef517189319326352b3f9` / CI `33895770619`, superseded frame-wire candidate `1be9dbbbd837067ee424bd942ab9efcef5a1a799` / CI `33894216176`, superseded corrective candidate `fccd2202deafbaebc091388149cc8d81b2f0801d` / CI `33892584822`, and older rejected candidate `ae87c229cd0a76a3b1611279ebcb3ef4ddd4d0fa` / CI `33888972744` are retained only as superseded evidence; local pre-push SBOM digests are not CI SBOM digests.

### Durable SBOM release assets

The dispatch-only workflow `.github/workflows/publish-sbom-release.yml` publishes durable, version-pinned release assets without changing runtime code. It requires an explicit full 40-hex `runtime_ref`, requires `release_tag` to match `upstream-vX.Y.Z`, derives release title/notes from that tag, checks out the runtime, runs `make security-check` (SBOM generation/validation, license self-test/review, vulnerability self-test/review), verifies `metadata.component.properties[rs-ai:vcs:revision]` equals the checked-out runtime SHA, normalizes assets to `dist/sbom.cdx.json` and `dist/sbom.cdx.json.sha256` with checksum filename `sbom.cdx.json`, and uploads replaceable assets with `--clobber` only after proving any existing release tag points to the same runtime. Fixed README links use `https://github.com/rcarmo/rs-ai/releases/download/upstream-vX.Y.Z/...` asset URLs for the accepted release; no `/latest` indirection is used. The previous `upstream-v0.85.0` release remains preserved; the workflow default now advances to `upstream-v0.85.1` for the accepted v0.85.1 runtime.

## Historical accepted release: v0.84.4

- Upstream package: `@earendil-works/pi-ai`
- Current audit target: `v0.84.4`
- Upstream tag/commit: `b79e4cc834970cca69daebffab7df1da7d1e52c4`
- Previous accepted upstream: `v0.84.3` / `4e58f324fae8ebfa98a3d45181fb248072a2afac`
- Accepted rs-ai baseline: `47befc1`
- Candidate rs-ai SHA: `caa03be5ddb0d7013d6576f3c17893522cc88849`
- Audited range: `4e58f324fae8ebfa98a3d45181fb248072a2afac..b79e4cc834970cca69daebffab7df1da7d1e52c4`
- Scope: `packages/ai` only; official tag/npm artifact only, no newer-main chase.
- Scope status: **complete for bounded deterministic v0.84.4 release parity**.

### v0.84.4 exact upstream path disposition

The official range changes **15** `packages/ai` paths: 2 package metadata paths, 2 scripts, 5 source/runtime/catalog paths, and 6 tests. Final corpus: **137** upstream `packages/ai/test/*.test.ts` files, recorded in `docs/v0844-137-test-crosswalk.md`.

Executable validator: `python3 scripts/validate_v0844_manifests.py` asserts this exact 15-path set and the 137 unique crosswalk rows.

| Status | Upstream path | rs-ai disposition |
|---|---|---|
| M | `packages/ai/CHANGELOG.md` | DOCUMENTED / metadata-only |
| M | `packages/ai/package.json` | DOCUMENTED / metadata-only |
| M | `packages/ai/scripts/generate-models.ts` | ADAPTED via extractor/generator and regenerated catalog |
| A | `packages/ai/scripts/openrouter-reasoning-options.ts` | ADAPTED in generator parity and OpenRouter reasoning tests |
| M | `packages/ai/src/api/mistral-conversations.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-completions.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/image-models.generated.ts` | ADAPTED by regenerated image registry + metadata verifier |
| M | `packages/ai/src/providers/cloudflare-ai-gateway.ts` | ADAPTED by generated Cloudflare AI Gateway catalog/routing evidence |
| M | `packages/ai/src/types.ts` | DOCUMENTED / option comment only; Rust option semantics already represented |
| M | `packages/ai/test/fireworks-models.test.ts` | ADAPTED / COVERED (see 137-test crosswalk) |
| M | `packages/ai/test/mistral-http-transport.test.ts` | ADAPTED / COVERED (see 137-test crosswalk) |
| M | `packages/ai/test/openai-completions-reasoning-details.test.ts` | ADAPTED / COVERED (see 137-test crosswalk) |
| M | `packages/ai/test/openai-completions-tool-choice.test.ts` | ADAPTED / COVERED (see 137-test crosswalk) |
| A | `packages/ai/test/openrouter-reasoning-options.test.ts` | ADAPTED / COVERED (see 137-test crosswalk) |
| M | `packages/ai/test/zai-coding-plan-models.test.ts` | ADAPTED / COVERED (see 137-test crosswalk) |

### v0.84.4 implementation summary

- Regenerated the text catalog from official npm `dist/providers/data` shards: **1290 text provider/id pairs across 39 providers and 9 APIs**; official OpenRouter batch aliases are now **40**. Full-record text delta from v0.84.3: `1312→1290`, `+57/-79/227 changed`.
- Regenerated the image catalog from the package `IMAGE_MODELS`: **50 image provider/id pairs**, including new OpenRouter image entries such as `meta/muse-image` and Recraft v4 vector/style variants. Full-record image delta from v0.84.3: `45→50`, `+5/0/0`.
- Updated release metadata verifier defaults to `@earendil-works/pi-ai@0.84.4` and pinned npm tarball SHA-256 `dfd3c929cee5a7387199a0a24dfc1be2096f1ea8f59ffb8285198a0ed01ebf93`.
- Preserved the exact v0.84.4 provider manifest shape: `schemaVersion=3`, `generatedAt=2026-08-28T22:00:02.569Z`, `structureHash=456b83c08bed3255d7e399d7927c6743e7f3568435691b3d38cc3666ffa70479`, `manifestSha256=904df30578689548238e080ca46be14b93ab6cd3de0d20445ee61eec11135474`.
- Ported OpenAI-compatible `reasoning_details` stream buffering: consecutive `reasoning.text` and `reasoning.summary` deltas merge before replay, while encrypted details remain discrete and are not duplicated.
- Confirmed provider-neutral `tool_choice: "none"` is emitted for OpenAI Completions and Responses even when no tools are present, without creating an empty `tools` field.
- Covered Mistral fragmented indexed tool-call chunks where later fragments omit `id` and carry an empty `function.name`; chunks merge by index and preserve the first id/name plus complete JSON arguments.
- Adapted OpenRouter reasoning metadata semantics: mandatory reasoning maps `off`/unsupported efforts to unavailable and omits reasoning for background calls; optional models still explicitly disable reasoning with `{ effort: "none" }`.
- Reflected generated catalog deltas: Fireworks turbo routers removed, Cloudflare AI Gateway mirrors Workers AI `/compat` models without duplicates, and ZAI Coding Plan CN `glm-5.3` pricing is updated.

Named Rust evidence added/updated for v0.84.4:

- `src/tests/release/v0844_release_test.rs::release_pinned_catalog_counts_match_v0844`
- `src/tests/release/v0844_release_test.rs::tool_choice_none_serializes_without_tools`
- `src/tests/release/v0844_release_test.rs::openrouter_mandatory_and_optional_reasoning_payloads_match_v0844`
- `src/tests/release/v0844_release_test.rs::cloudflare_workers_ai_models_are_mirrored_into_gateway_compat_catalog`
- `src/tests/release/v0844_release_test.rs::zai_coding_plan_glm_5_3_cost_matches_v0844`
- `src/tests/release/v0844_release_test.rs::mistral_indexed_tool_call_fragments_merge_without_repeated_ids_or_names`
- `src/tests/providers/openai/openai_completions_reasoning_details_test.rs::merges_adjacent_text_and_summary_reasoning_details_before_replay`
- `src/tests/catalogs/fireworks_models_test.rs::omits_removed_fire_pass_turbo_router_models`
- `src/tests/release/release_metadata_verification_test.rs::v0844_manifest_validator_confirms_changed_paths_and_crosswalk_rows`

### v0.84.4 release-pinned artifact evidence

- upstream tag worktree: `/workspace/tmp/pi-src` at `b79e4cc834970cca69daebffab7df1da7d1e52c4`
- candidate rs-ai SHA: `caa03be5ddb0d7013d6576f3c17893522cc88849`
- npm tarball SHA-256: `dfd3c929cee5a7387199a0a24dfc1be2096f1ea8f59ffb8285198a0ed01ebf93`
- provider manifest structure hash: `456b83c08bed3255d7e399d7927c6743e7f3568435691b3d38cc3666ffa70479`
- extracted release JSON: `/workspace/tmp/pi-v0844-json/models.json`
- hosted CI: GitHub Actions run `33252334207` completed with conclusion `success` for candidate `caa03be5ddb0d7013d6576f3c17893522cc88849`; job `build-test-lint` (`99100057309`) succeeded, including `Build`, `Clippy (deny warnings)`, and `Test` steps (`https://github.com/rcarmo/rs-ai/actions/runs/33252334207/job/99100057309`).

Commands/results run so far:

```bash
python3 scripts/validate_release_model_data.py <v0.84.4 package>/dist/providers/data
python3 scripts/extract_release_model_shards.py <v0.84.4 package> /workspace/tmp/pi-v0844-json --tag-worktree /workspace/tmp/pi-src --tag-sha b79e4cc834970cca69daebffab7df1da7d1e52c4
python3 scripts/verify_release_model_metadata.py
python3 scripts/validate_v0844_manifests.py
cargo test v0844_release_test -- --nocapture
cargo test openai_completions_reasoning_details_test -- --nocapture
cargo test --all-targets --all-features
```

Final local results: `cargo fmt -- --check` passed; `cargo build` passed; metadata verifier `metadata verified: text=1290 providers=39 apis=9 batchAliases=40 image=50`; deliberate `--fault text-name` and `--fault image-name` metadata gates failed with the expected text/image mismatches; provider/id comparator passed with text `upstream=1290 local=1290 missing=0 extra=0` and image `upstream=50 local=50 missing=0 extra=0`; v0.84.4 manifest validator `changedPaths=15 testRows=137`; focused v0.84.4 tests `6 passed`; OpenAI reasoning-details tests `4 passed`; full `cargo test --all-targets --all-features` passed three consecutive times (`936 passed`, `0 failed`, `0 ignored`); strict `cargo clippy --all-targets -- -D warnings` passed. Hosted GitHub Actions run `33252334207` also completed successfully for candidate `caa03be5ddb0d7013d6576f3c17893522cc88849`, with job `build-test-lint` (`99100057309`) succeeding all Build, Clippy, and Test steps.

## Policy/SBOM maintenance: 2026-08-29

Post-acceptance policy convergence commit for the accepted v0.84.4 line adds executable supply-chain gates without changing runtime code or generated catalogs.

- SBOM generator: repo-local `scripts/sbom.py` (`rs-ai-sbom.py` version `1.0.0`), consuming `cargo metadata --locked --all-features` and committed `Cargo.lock`.
- SBOM output: gitignored `artifacts/sbom.cdx.json` plus `artifacts/sbom.cdx.json.sha256`; local/CI generation contains **283** third-party dependency components.
- SBOM digest evidence model: the SBOM embeds the root git revision, so static in-tree digest values are intentionally not recorded. Treat the final CI `rs-ai-sbom` artifact and its uploaded `.sha256` file as authoritative for the tested candidate SHA; evidence-only follow-up commits must record both the tested candidate SHA and docs SHA if a digest is copied into `RELEASE.md` later.
- SBOM validation: `make sbom && make sbom-check` passed, validating CycloneDX fields, root crate/revision, dependency list, checksum, stale output, malformed/empty output, and absence of local paths/secrets.
- Vulnerability scanner: pinned `cargo-audit 0.22.2`; `make vuln-check-selftest` passed fail-closed wrapper tests for scanner error exit + empty JSON, malformed/incomplete reports, unapproved advisory, expired/incomplete waiver, and approved advisory; `make vuln-check` passed with temporary owner-approved AWS legacy transitive exceptions with mitigation and expiry `2026-09-30` for `h2 0.3.27` (`RUSTSEC-2026-0258`) and `rustls-webpki 0.101.7` (`RUSTSEC-2026-0098`, `RUSTSEC-2026-0099`, `RUSTSEC-2026-0104`).
- License review: `make license-check-selftest` passed fail-closed SPDX expression tests for MIT, Apache-2.0 OR MIT, MIT AND Apache-2.0, MIT AND unknown, MIT AND GPL, proprietary `LicenseRef-*`, permissive OR with a bad branch, legacy MIT/Apache slash separators, malformed, and missing expressions; `make license-check` passed for **283** third-party packages using the committed allowlist of permissive tokens.
- CI retention: `.github/workflows/ci.yml` uploads `artifacts/sbom.cdx.json` and `artifacts/sbom.cdx.json.sha256` as `rs-ai-sbom-${{ github.sha }}` with `if-no-files-found: error` and `retention-days: 30`; scheduled CI runs weekly (`17 4 * * 1`) for independent SBOM/license/RustSec maintenance between releases. `.github/workflows/prune-actions-artifacts.yml` protects `rs-ai-sbom-*` artifacts and their associated workflow runs from cleanup for the same 30-day SBOM evidence window.
- Lockfile decision: `Cargo.lock` is now committed for reproducible library SBOM/security resolution; generated SBOM artifacts remain uncommitted.
- Local gate: `make check` passed after these policy/workflow changes, including fmt, build, strict all-feature Clippy, all-target all-feature tests (`936 passed`, `0 failed`, `0 ignored`), SBOM validation, license wrapper self-tests/review, vulnerability wrapper self-tests including missing/empty mitigation checks, and vulnerability scan. CI now includes an explicit `cargo fmt --all -- --check` step, runs `make sbom && make sbom-check` so fresh checkouts generate artifacts before validation while `sbom-check` remains validation-only for stale/malformed artifact detection, and runs license/vulnerability wrapper self-tests before the pinned reviews.

## Historical accepted release: v0.84.3

- Upstream package: `@earendil-works/pi-ai`
- Historical accepted release: `v0.84.3`
- Upstream tag/commit: `4e58f324fae8ebfa98a3d45181fb248072a2afac`
- Previous accepted upstream: `v0.84.2` / `914cf1472e715297caa30db4b9535d534a9eb718`
- Accepted rs-ai baseline: `f615a0b525bd06b75e61de3dd87cab6dc497cf00`
- Audited range: `914cf1472e715297caa30db4b9535d534a9eb718..4e58f324fae8ebfa98a3d45181fb248072a2afac`
- Scope: `packages/ai` only; official tag only, no newer-main chase.
- Scope status: **complete for bounded deterministic v0.84.3 release parity**.

### v0.84.3 exact upstream path disposition

The official range changes **48** `packages/ai` paths: 19 source/runtime paths (18 modified plus added `src/utils/sleep.ts`), 25 tests (20 modified plus 5 added), and 4 package/docs/export paths. Final corpus: **136** upstream `packages/ai/test/*.test.ts` files, recorded in `docs/v0843-136-test-crosswalk.md`.

Executable validator: `python3 scripts/validate_v0843_manifests.py` asserts this exact 48-path set and the 136 unique crosswalk rows.

| Status | Upstream path | rs-ai disposition |
|---|---|---|
| M | `packages/ai/CHANGELOG.md` | DOCUMENTED / metadata-only |
| M | `packages/ai/README.md` | DOCUMENTED / metadata-only |
| M | `packages/ai/package.json` | DOCUMENTED / metadata-only |
| M | `packages/ai/scripts/generate-models.ts` | ADAPTED via extractor/generator and regenerated catalog |
| M | `packages/ai/src/api/anthropic-messages.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/azure-openai-responses.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/bedrock-converse-stream.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/google-generative-ai.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/google-shared.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/google-vertex.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/mistral-conversations.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-codex-responses.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-completions.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-responses.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/pi-messages.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/simple-options.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/auth/oauth/device-code.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/auth/oauth/github-copilot.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/auth/oauth/kimi-coding.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/index.ts` | N/A TypeScript export surface; Rust exposes modules directly |
| M | `packages/ai/src/providers/xai.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/types.ts` | ADAPTED in Rust provider/runtime code |
| A | `packages/ai/src/utils/sleep.ts` | ADAPTED via cancellation-aware Rust retry sleep primitives |
| M | `packages/ai/test/anthropic-auth-token.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/azure-openai-base-url.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| A | `packages/ai/test/azure-openai-tool-choice.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/baseten-models.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| A | `packages/ai/test/bedrock-redacted-reasoning.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| A | `packages/ai/test/bedrock-response-headers.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/generate-models-strict.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/github-copilot-oauth.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/google-raw-stop-reason.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| A | `packages/ai/test/google-thinking-level-map.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/google-vertex-api-key-resolution.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/mistral-http-transport.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/model-catalog-types.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/openai-completions-reasoning-details.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/openai-completions-thinking-as-text.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/openai-completions-thinking-token-budget.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/openai-completions-tool-choice.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/openai-completions-tool-result-images.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/pi-messages.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/qwen-token-plan-models.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/stream.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/supports-xhigh.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/xai-responses.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| M | `packages/ai/test/xiaomi-models.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |
| A | `packages/ai/test/zai-coding-plan-models.test.ts` | ADAPTED / COVERED (see 136-test crosswalk) |

### v0.84.3 implementation summary

- Regenerated the text catalog from official npm `dist/providers/data` shards: **1312 text provider/id pairs across 39 providers and 9 APIs**; official batch aliases remain **60**.
- Verified image catalog remains **45 image provider/id pairs**.
- Updated release metadata verifier defaults to `@earendil-works/pi-ai@0.84.3` and pinned npm tarball SHA-256 `9c40af2f43950f8e94e7bbcd0c1b3548f000972da00c4fb9c0d0529d4d7d5431`.
- Extended exact Qwen/ZAI Coding Plan Individual allowlist with `deepseek-v4-pro-0813`; regenerated xAI/ZAI/Xiaomi compatibility metadata.
- Added provider-neutral `tool_choice` forwarding for Responses/Azure Responses while preserving tool definitions; ported Anthropic server-side fallback beta/payload, fallback response-model capture, and fallback-model usage pricing.
- Added default Pi User-Agent on OpenAI-compatible, Responses/Azure, Anthropic Messages, Google, and Mistral HTTP adapters, with explicit request headers overriding defaults.
- Added Google-specific thinking-level resolver for v0.84.3: standard levels preserve exact mapping, xhigh/max must map through `thinkingLevelMap`, unsupported mappings error, and mapped token budgets are honored for Google/Vertex payloads.
- Added Bedrock redacted reasoning replay support (`reasoningContent.redactedContent`) and stream finalization for open/redacted reasoning blocks; documented the AWS Rust SDK `ConverseStreamOutput` raw-header boundary and added executable status/request-id onResponse adaptation evidence.
- Preserved OpenAI-compatible full `reasoning_details` arrays on thinking signatures and replays them in request payloads; encrypted tool-call detail fallback remains covered.
- Added Copilot policy update filtering for known/tool-capable/unconfigured models and one Retry-After retry for throttled policy updates while continuing best-effort after transport failures.
- Documented TS export/package/docs-only changes as N/A for Rust runtime.

Named Rust evidence added/updated for v0.84.3:

- `src/tests/release/v0843_release_test.rs::release_pinned_catalog_counts_match_v0843`
- `src/tests/release/v0843_release_test.rs::google_thinking_level_resolver_matches_v0843`
- `src/tests/release/v0843_release_test.rs::google_payload_uses_mapped_levels_and_token_budgets`
- `src/tests/release/v0843_release_test.rs::azure_responses_payload_forwards_provider_neutral_tool_choice`
- `src/tests/release/v0843_release_test.rs::openai_responses_uses_pi_user_agent_by_default_and_allows_override`
- `src/tests/release/v0843_release_test.rs::azure_responses_uses_pi_user_agent_and_preserves_tool_choice_on_wire`
- `src/tests/release/v0843_release_test.rs::completions_anthropic_and_mistral_default_user_agent_can_be_overridden`
- `src/tests/providers/anthropic/anthropic_fallback_test.rs::{anthropic_payload_includes_server_side_fallbacks,anthropic_stream_sends_fallback_beta_and_prices_response_model_usage}`
- `src/tests/providers/bedrock/bedrock_error_metadata_test.rs::on_response_metadata_adapts_sdk_exposed_status_and_request_id_boundary`
- `src/tests/providers/xai/xai_grok45_responses_test.rs::xai_grok_46_uses_responses_xhigh_encrypted_reasoning_and_user_agent_override`
- `src/tests/providers/bedrock/bedrock_thinking_payload_test.rs::replays_redacted_reasoning_as_bedrock_redacted_content`, `src/tests/providers/bedrock/bedrock_error_metadata_test.rs::on_response_metadata_adapts_sdk_exposed_status_and_request_id_boundary`
- `src/tests/providers/openai/openai_completions_reasoning_details_test.rs::{preserves_streamed_text_and_summary_reasoning_details_on_thinking,replays_thinking_signature_reasoning_details_sequence}`
- `src/tests/auth/oauth/github_copilot_oauth_test.rs::{policy_updates_only_known_tool_capable_unconfigured_models,retries_throttled_policy_update_once_and_continues_transport_failures}`
- `src/tests/release/release_metadata_verification_test.rs::v0843_manifest_validator_confirms_changed_paths_and_crosswalk_rows`

### v0.84.3 release-pinned artifact evidence

- upstream tag worktree: `/workspace/tmp/pi-src` at `4e58f324fae8ebfa98a3d45181fb248072a2afac`
- npm tarball SHA-256: `9c40af2f43950f8e94e7bbcd0c1b3548f000972da00c4fb9c0d0529d4d7d5431`
- provider manifest structure hash: `9a017e31c46be9520da694c2d30f95ddb3e4efa885bd043615d5b6f48e90eb81`
- extracted release JSON: `/workspace/tmp/pi-v0843-json/models.json`

Commands/results run so far:

```bash
python3 scripts/validate_release_model_data.py <v0.84.3 package>/dist/providers/data
python3 scripts/extract_release_model_shards.py <v0.84.3 package> /workspace/tmp/pi-v0843-json --tag-worktree /workspace/tmp/pi-src --tag-sha 4e58f324fae8ebfa98a3d45181fb248072a2afac
python3 scripts/verify_release_model_metadata.py
python3 scripts/validate_v0843_manifests.py
cargo test v0843_release_test -- --nocapture
cargo test github_copilot_oauth_test -- --nocapture
cargo test bedrock -- --nocapture
cargo test openai_completions_reasoning_details_test -- --nocapture
cargo test --all-targets --all-features
```

Final local results: validator `1312` models / `39` providers; metadata verifier `metadata verified: text=1312 providers=39 apis=9 batchAliases=60 image=45`; deliberate text/image metadata faults both failed with expected mismatches; v0.84.2 manifest validator `changedPaths=42 testRows=131`; v0.84.3 manifest validator `changedPaths=48 testRows=136`; focused v0.84.3 tests `7 passed`; Copilot OAuth focused tests `7 passed`; Bedrock focused tests `56 passed`; OpenAI reasoning-details tests (`cargo test openai_completions_reasoning_details_test -- --nocapture`) `3 passed`; Anthropic fallback tests `2 passed`; Bedrock metadata tests `8 passed`; xAI Responses tests `3 passed`; metadata/manifest verifier tests `5 passed`; `cargo build` passed; full `cargo test --all-targets --all-features` passed three consecutive times (`928` tests, `0` failed, `0` ignored); strict Clippy passed with both `--all-targets --all-features` and `--all-targets`. Hosted CI evidence pending until after push.

## Historical accepted release: v0.84.2

- Upstream package: `@earendil-works/pi-ai`
- Historical accepted release: `v0.84.2`
- Upstream tag/commit: `914cf1472e715297caa30db4b9535d534a9eb718`
- Previous accepted upstream: `v0.84.1` / `53fa77ccd8a279eb87e92294ef3687b03ff80112`
- Audited range: `53fa77ccd8a279eb87e92294ef3687b03ff80112..914cf1472e715297caa30db4b9535d534a9eb718`
- Scope: `packages/ai` only; official tag only, no newer-main chase.
- Scope status: **complete for bounded deterministic v0.84.2 release parity**.

### v0.84.2 exact upstream path disposition

The official range changes **42** `packages/ai` paths: 18 source, 21 tests, 3 package/docs. Test accounting: **21 changed test paths total = 18 modified existing tests + 3 new tests**. Final corpus: **131** upstream `packages/ai/test/*.test.ts` files, recorded in `docs/v0842-131-test-crosswalk.md`.

Executable validator: `python3 scripts/validate_v0842_manifests.py` asserts this exact 42-path set and the 131 unique crosswalk rows.

| Status | Upstream path | rs-ai disposition |
|---|---|---|
| M | `packages/ai/CHANGELOG.md` | DOCUMENTED / metadata-only |
| M | `packages/ai/package.json` | DOCUMENTED / metadata-only |
| M | `packages/ai/scripts/generate-models.ts` | ADAPTED in extractor/generator scripts |
| M | `packages/ai/src/api/anthropic-messages.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/bedrock-converse-stream.ts` | ADAPTED in Rust provider/runtime code |
| A | `packages/ai/src/api/cloudflare-gateway-binding.ts` | N/A Workers binding object; Rust HTTP routing adaptation covered |
| M | `packages/ai/src/api/constrained-sampling.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/google-generative-ai.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/google-shared.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/google-vertex.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/mistral-conversations.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-codex-responses.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-completions.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-responses-shared.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/api/openai-responses.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/auth/oauth/github-copilot.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/image-models.generated.ts` | ADAPTED by regenerated image registry + metadata verifier |
| M | `packages/ai/src/types.ts` | ADAPTED in Rust types |
| A | `packages/ai/src/utils/pi-user-agent.ts` | ADAPTED via pi_runtime_user_agent |
| M | `packages/ai/src/utils/retry.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/src/utils/validation.ts` | ADAPTED in Rust provider/runtime code |
| M | `packages/ai/test/anthropic-auth-token.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/anthropic-eager-tool-input-compat.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/bedrock-convert-messages.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| A | `packages/ai/test/cloudflare-gateway-binding.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/constrained-sampling.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/context-overflow.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/deferred-tools.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/github-copilot-oauth.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/google-raw-stop-reason.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/lazy-module-load.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| A | `packages/ai/test/mistral-http-transport.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/mistral-raw-stop-reason.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/openai-codex-stream.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/openai-completions-tool-choice.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/openai-responses-compat.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| A | `packages/ai/test/openai-responses-namespace.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/retry.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/stream.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/supports-xhigh.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/total-tokens.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |
| M | `packages/ai/test/validation.test.ts` | ADAPTED / COVERED (see 131-test crosswalk) |

Changed test paths:

- Modified: `anthropic-auth-token`, `anthropic-eager-tool-input-compat`, `bedrock-convert-messages`, `constrained-sampling`, `context-overflow`, `deferred-tools`, `github-copilot-oauth`, `google-raw-stop-reason`, `lazy-module-load`, `mistral-raw-stop-reason`, `openai-codex-stream`, `openai-completions-tool-choice`, `openai-responses-compat`, `retry`, `stream`, `supports-xhigh`, `total-tokens`, `validation`.
- Added: `cloudflare-gateway-binding`, `mistral-http-transport`, `openai-responses-namespace`.

### v0.84.2 implementation summary

- Regenerated text catalog from official npm `dist/providers/data` shards: **1267 text provider/id pairs across 39 providers and 9 APIs**; official batch aliases now **60**.
- Regenerated image catalog: **45 image provider/id pairs**.
- Updated release metadata verifier defaults to `@earendil-works/pi-ai@0.84.2` and pinned npm tarball SHA-256 `0262785a76b0eb2eec596cd8a7ab2ee23eef89d2ef1bb1211c4f0a1944dacf41`.
- Extended audited batch-alias policy for the v0.84.2 artifact (`openrouter/google/gemini-3.7-flash:batch` added) while preserving fail-closed behavior.
- Ported strict JSON-schema tool conversion for OpenAI-compatible tools and optional non-nullable null omission during validation.
- Added Kimi/Codex `pi (${platform} ${release}; ${arch})` runtime User-Agent handling.
- Added Responses/Codex message-anchored `additional_tools` selection, retaining tool-search/top-level fallbacks.
- Added tool-call `namespace` and assistant-message `endTurn` fields; namespace is replayed only when additional-tools replay is supported.
- Updated DeepSeek URL detection to be case-insensitive and to use `max_tokens`.
- Updated Google/Vertex finish handling so `MAX_TOKENS` with tool calls remains `length`, while `STOP` with tool calls becomes `toolUse`.
- Added retry classifier coverage for `exceeded request buffer limit`.
- Added focused Mistral HTTP/SSE transport coverage for delayed incremental chunks, split UTF-8 bytes, cancellation/drop cleanup, chunk timeouts, bounded 403 error bodies, retry body replay, affinity override/suppression, exact request payload/header replay, thinking/text/tool calls, raw stop reason, and cached-token usage.
- Ported GitHub Copilot policy parsing/fetching and bounded best-effort policy-enable batching (`COPILOT_POLICY_CONCURRENCY = 4`) with deterministic local HTTP tests; assessed Cloudflare `createGatewayBindingFetch` as **N/A** for Rust Workers binding object semantics; Rust transport injection has no Cloudflare Workers binding object, while existing Cloudflare gateway HTTP routing/header tests remain applicable.

Named Rust evidence:

- `src/tests/release/v0842_release_test.rs::release_pinned_catalog_counts_match_v0842`
- `src/tests/release/v0842_release_test.rs::strict_json_schema_tools_require_optional_properties_as_nullable`
- `src/tests/release/v0842_release_test.rs::optional_non_nullable_null_is_omitted_but_nullable_null_is_preserved`
- `src/tests/release/v0842_release_test.rs::deepseek_detection_is_case_insensitive_and_uses_max_tokens`
- `src/tests/release/v0842_release_test.rs::retry_classifier_matches_request_buffer_exhaustion_wording`
- `src/tests/release/v0842_release_test.rs::pi_runtime_user_agent_includes_platform_release_and_arch`
- `src/tests/release/v0842_release_test.rs::responses_additional_tools_supersedes_tool_search_for_deferred_tools`
- `src/tests/release/v0842_release_test.rs::responses_replays_namespace_only_when_additional_tools_supported`
- `src/tests/release/v0842_release_test.rs::mistral_http_sse_parses_utf8_usage_and_raw_tool_stop`
- `src/tests/release/v0842_release_test.rs::mistral_http_stream_yields_delayed_chunks_incrementally`
- `src/tests/release/v0842_release_test.rs::mistral_http_stream_preserves_utf8_split_across_byte_chunks`
- `src/tests/release/v0842_release_test.rs::mistral_http_stream_cancel_while_waiting_for_chunk_cleans_up`
- `src/tests/release/v0842_release_test.rs::mistral_http_stream_timeout_while_awaiting_chunk_reports_error`
- `src/tests/release/v0842_release_test.rs::mistral_http_uses_bounded_branded_error_body_for_403`
- `src/tests/release/v0842_release_test.rs::mistral_http_retries_with_replayable_json_body`
- `src/tests/release/v0842_release_test.rs::mistral_http_affinity_override_and_suppression_are_honored`
- `src/tests/release/v0842_release_test.rs::mistral_http_exact_wire_payload_matches_replay_contract`
- `src/tests/auth/oauth/github_copilot_oauth_test.rs::filters_models_to_the_authenticated_account_picker_catalog`
- `src/tests/auth/oauth/github_copilot_oauth_test.rs::is_selectable_requires_picker_enabled_not_disabled_and_tool_calls`
- `src/tests/auth/oauth/github_copilot_oauth_test.rs::falls_back_to_policy_enabled_ids_only_for_individual_endpoint`
- `src/tests/auth/oauth/github_copilot_oauth_test.rs::fetch_available_model_ids_uses_copilot_headers`
- `src/tests/auth/oauth/github_copilot_oauth_test.rs::limits_concurrent_policy_updates_to_four_during_login`
- `src/tests/release/release_metadata_verification_test.rs::v0842_manifest_validator_confirms_changed_paths_and_crosswalk_rows`


### v0.84.2 release-pinned artifact evidence

- upstream tag worktree: `/workspace/tmp/pi-src` at `914cf1472e715297caa30db4b9535d534a9eb718`
- unpacked npm package: `/workspace/tmp/pi-ai-0842-pkg/package`
- npm tarball SHA-256: `0262785a76b0eb2eec596cd8a7ab2ee23eef89d2ef1bb1211c4f0a1944dacf41`
- provider manifest `schemaVersion=3`, `generatedAt=2026-08-14T10:02:30.583Z`, `structureHash=012f19b2e2c92706bc700f4c9dd80a21f1f43d68959bc90c78d2cdb51374d5cc`
- extracted release JSON: `/workspace/tmp/pi-v0842-json/models.json`

Commands/results run so far:

```bash
python3 scripts/validate_release_model_data.py /workspace/tmp/pi-ai-0842-pkg/package/dist/providers/data
python3 scripts/extract_release_model_shards.py /workspace/tmp/pi-ai-0842-pkg/package /workspace/tmp/pi-v0842-json --tag-worktree /workspace/tmp/pi-src --tag-sha 914cf1472e715297caa30db4b9535d534a9eb718
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0842-json python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 914cf1472e715297caa30db4b9535d534a9eb718
python3 scripts/verify_release_model_metadata.py
cargo test v0842_release_test -- --nocapture
cargo test github_copilot_oauth_test -- --nocapture
cargo test release_metadata_verification_test -- --nocapture
python3 scripts/validate_v0842_manifests.py
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

Results: validator `1267` models / `39` providers; extractor `1267` models / `39` providers / `9` APIs / `60` batch aliases; pair comparator text `1267/1267`, image `45/45`, missing `0`, extra `0`; full metadata verifier `metadata verified: text=1267 providers=39 apis=9 batchAliases=60 image=45`; deliberate text/image metadata faults both failed with the expected comparator mismatch; `python3 scripts/validate_v0842_manifests.py` passed (`changedPaths=42 testRows=131`); `cargo build` passed; focused v0.84.2 tests `17 passed`; focused Copilot OAuth tests `5 passed`; metadata/manifest verifier tests `4 passed`; full `cargo test --all-targets --all-features` passed three consecutive times (`911` tests, `0` failed, `0` ignored); strict Clippy passed with both `--all-targets --all-features` and `--all-targets`. Hosted CI evidence pending until after push.

## Historical accepted release: v0.84.1

- Upstream package: `@earendil-works/pi-ai`
- Current accepted release: `v0.84.1`
- Upstream tag/commit: `53fa77ccd8a279eb87e92294ef3687b03ff80112`
- Previous accepted upstream: `v0.84.0` / `a5f43bf8aff3c55752432655f7334e3dafd1e256`
- Audited range: `a5f43bf8aff3c55752432655f7334e3dafd1e256..53fa77ccd8a279eb87e92294ef3687b03ff80112`
- Scope: `packages/ai` only; official tag only, no newer-main chase.
- Scope status: **complete for bounded deterministic v0.84.1 release parity**.

### v0.84.1 exact upstream path disposition

The official range changes **25** `packages/ai` paths:

| Path group | Count | Disposition |
|---|---:|---|
| Release/package/docs (`CHANGELOG.md`, `README.md`, `package.json`) | 3 | DOCUMENTED / N/A runtime; ledger updated, README env semantics mirrored. |
| Generator/model-data scripts (`scripts/generate-models.ts`, `scripts/model-data.ts`) | 2 | ADAPTED via release-shard extractor strict allowlist checks and production validator tests. |
| Runtime/provider/type/registry (`src/env-api-keys.ts`, `src/models.generated.ts`, `src/providers/all.ts`, `src/providers/qwen-token-plan-individual*.ts`, `src/types.ts`) | 6 | PORTED: new `qwen-token-plan-individual` provider, env mapping, generated catalog, provider/id pairs, and Qwen reasoning payload behavior. |
| Tests | 14 | ADAPTED / LIVE UNEXECUTED where credential-gated. Correct accounting: **13 existing tests modified + 1 new `generate-models-strict.test.ts`**; exact split is **10 credential-gated live-matrix additions + 2 deterministic provider/request rows + 2 generator-policy rows**. Full 128-file corpus recorded in `docs/v0841-128-test-crosswalk.md`. |

Changed paths:

- M `packages/ai/CHANGELOG.md`
- M `packages/ai/README.md`
- M `packages/ai/package.json`
- M `packages/ai/scripts/generate-models.ts`
- M `packages/ai/scripts/model-data.ts`
- M `packages/ai/src/env-api-keys.ts`
- M `packages/ai/src/models.generated.ts`
- M `packages/ai/src/providers/all.ts`
- A `packages/ai/src/providers/qwen-token-plan-individual.models.ts`
- A `packages/ai/src/providers/qwen-token-plan-individual.ts`
- M `packages/ai/src/types.ts`
- M `packages/ai/test/abort.test.ts`
- M `packages/ai/test/context-overflow.test.ts`
- M `packages/ai/test/cross-provider-handoff.test.ts`
- M `packages/ai/test/empty.test.ts`
- A `packages/ai/test/generate-models-strict.test.ts`
- M `packages/ai/test/image-tool-result.test.ts`
- M `packages/ai/test/model-data-validation.test.ts`
- M `packages/ai/test/openai-completions-tool-choice.test.ts`
- M `packages/ai/test/qwen-token-plan-models.test.ts`
- M `packages/ai/test/stream.test.ts`
- M `packages/ai/test/tokens.test.ts`
- M `packages/ai/test/tool-call-without-result.test.ts`
- M `packages/ai/test/total-tokens.test.ts`
- M `packages/ai/test/unicode-surrogate.test.ts`

### v0.84.1 implementation summary

- Added/released `qwen-token-plan-individual` as a built-in text provider using the shared international endpoint `https://token-plan.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1` and shared env key `QWEN_TOKEN_PLAN_API_KEY`.
- Regenerated the text catalog from official npm `dist/providers/data` shards: **1220 text provider/id pairs across 39 providers and 9 APIs**.
- Preserved the exact seven Individual models: `deepseek-v4-flash-0731`, `deepseek-v4-pro`, `glm-5.2`, `qwen3.6-flash`, `qwen3.7-max`, `qwen3.7-plus`, `qwen3.8-max`; retired `qwen3.8-max-preview` remains omitted.
- Fixed OpenAI-compatible `thinkingFormat = "qwen"` request building to emit `reasoning_effort` when `supportsReasoningEffort` is true while continuing to emit top-level `enable_thinking` and no `thinking` object.
- Updated `scripts/extract_release_model_shards.py` for v0.84.1 release artifacts: official npm shards now include **59** OpenRouter `:batch` aliases. The extractor preserves only the exact audited allowlist, rejects any unexpected batch alias, records `batchAliasCount`, `batchAliases`, and `allowedBatchAliasPolicySha256`, and enforces the Qwen Individual model ID allowlist before creating output.
- Added `scripts/verify_release_model_metadata.py`, a clean-run full metadata gate that downloads the official npm package, verifies tarball SHA-256 `6ab689189e7cb3de5cdb126312a3e60e8ac35fe5ee5f1b63d00f711c8a430c73` before extraction, validates/extracts shards, imports package image metadata, regenerates text and image Rust registries in a temporary project copy, rustfmt-formats the generated outputs, normalizes only generated timestamps, and compares all Rust-representable metadata byte-for-byte against committed files.
- Added deterministic Rust evidence:
  - `src/tests/release/v0841_release_test.rs::release_pinned_catalog_counts_include_individual_and_batch_aliases`
  - `src/tests/release/v0841_release_test.rs::qwen_token_plan_individual_catalog_env_and_endpoint_match_v0841`
  - `src/tests/release/v0841_release_test.rs::qwen_token_plan_individual_reasoning_payloads_match_v0841`
  - `src/tests/catalogs/model_data_validation_test.rs::extractor_enforces_qwen_individual_strict_model_ids_without_output_mutation`
  - `src/tests/catalogs/model_data_validation_test.rs::extractor_allows_only_audited_release_batch_aliases`
  - `src/tests/release/release_metadata_verification_test.rs::release_metadata_verifier_clean_run_succeeds_with_expected_counts`
  - `src/tests/release/release_metadata_verification_test.rs::release_metadata_verifier_detects_fault_injected_text_metadata`

### v0.84.1 release-pinned artifact evidence

Authoritative catalog source is offline and release-pinned:

- upstream tag worktree: `/workspace/tmp/pi-src` at `53fa77ccd8a279eb87e92294ef3687b03ff80112`
- unpacked npm package: `/workspace/tmp/pi-ai-0841-pkg/package`
- provider shards: `/workspace/tmp/pi-ai-0841-pkg/package/dist/providers/data/*.json`
- provider manifest `schemaVersion=3`, `generatedAt=2026-08-07T05:53:06.539Z`, `structureHash=24c74ac10bb8ed4df2c96bdadcfd94a417f3c823d5038875f59a261e3c84424b`
- extracted release JSON: `/workspace/tmp/pi-v0841-json/models.json`
- extractor metadata: `/workspace/tmp/pi-v0841-json/source-metadata.json`

Commands:

```bash
python3 scripts/validate_release_model_data.py /workspace/tmp/pi-ai-0841-pkg/package/dist/providers/data
python3 scripts/extract_release_model_shards.py /workspace/tmp/pi-ai-0841-pkg/package /workspace/tmp/pi-v0841-json --tag-worktree /workspace/tmp/pi-src --tag-sha 53fa77ccd8a279eb87e92294ef3687b03ff80112
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0841-json python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 53fa77ccd8a279eb87e92294ef3687b03ff80112
python3 scripts/verify_release_model_metadata.py
```

Results:

- Validator: `{"models": 1220, "providers": 39, "structureHash": "24c74ac10bb8ed4df2c96bdadcfd94a417f3c823d5038875f59a261e3c84424b"}`
- Extractor: `1220 models, 39 providers, 9 apis`, `batchAliasCount=59`, `allowedBatchAliasPolicySha256=f383057c43e309e6882645d1f294b5e539fa8521209c24d43e9390af0d7d8281`
- Pair comparator: text `1220/1220`, image `42/42`, missing `0`, extra `0`
- Full metadata verifier: verifies npm tarball SHA-256 `6ab689189e7cb3de5cdb126312a3e60e8ac35fe5ee5f1b63d00f711c8a430c73` before extraction and reports derived counts `metadata verified: text=1220 providers=39 apis=9 batchAliases=59 image=42`

### v0.84.1 verification

Focused evidence:

```bash
cargo fmt --check
python3 scripts/validate_release_model_data.py /workspace/tmp/pi-ai-0841-pkg/package/dist/providers/data
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0841-json python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 53fa77ccd8a279eb87e92294ef3687b03ff80112
python3 scripts/verify_release_model_metadata.py
cargo test v0841_release_test -- --nocapture
cargo test model_data_validation_test -- --nocapture
cargo test release_metadata_verification_test -- --nocapture
```

Results: format clean; validator/comparator clean; full metadata verifier clean with pinned tarball SHA and derived image count; `v0841_release_test` `3 passed`; `model_data_validation_test` `3 passed`; metadata verifier native tests `2 passed` (clean success + fault injection).

Full gates from `/workspace/projects/rs-ai`:

```bash
cargo build
cargo test        # pass 1/3
cargo test        # pass 2/3
cargo test        # pass 3/3
cargo clippy --all-targets -- -D warnings
```

Results: build passed; full test suite passed three consecutive times (`887` tests plus doctest `1` before adding metadata verifier tests; `890` native tests afterward); strict Clippy passed. Final correction gates also include `cargo fmt --check`, clean metadata verifier, `cargo test release_metadata_verification_test -- --nocapture` (`2 passed`), `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo clippy --all-targets --all-features -- -D warnings`. Hosted CI initially exposed a newer Clippy `useless-borrows-in-formatting` lint in `src/provider/codex.rs`; the redundant borrow was removed and native CI-equivalent Clippy is now clean. Hosted CI also lacks Bun, so the metadata verifier now falls back to Node for package ESM import, and the stale structural catalog count assertion was updated to current v0.84.1 counts (`1220`/`39`).

## Historical prior release: v0.84.0

- Upstream package: `@earendil-works/pi-ai`
- Historical accepted release: `v0.84.0`
- Upstream tag/commit: `a5f43bf8aff3c55752432655f7334e3dafd1e256`
- Previous accepted upstream: `v0.83.0` / `845d6ff1f6643aba440341cce877ce1c43ebbc39`
- Audited range for manifests: `845d6ff1f6643aba440341cce877ce1c43ebbc39..a5f43bf8aff3c55752432655f7334e3dafd1e256`
- Scope: `packages/ai` only; no newer-main chase.
- Scope status: **complete for bounded deterministic v0.84.0 release parity**. Deterministic changed assertions are ported/adapted, covered by existing Rust tests, or explicitly N/A for live credentials/interactive UI/JS-runtime-only surfaces.

### Exact manifests generated

- `docs/v0840-manifests.md` records the exact **101** changed `packages/ai` paths and **46** changed `packages/ai/test` files, with extracted upstream case/assertion/gate lines from the authoritative tag.

### Slice 1: Baseten / sampling / vLLM budget

Ported/adapted in this slice:

- Regenerated text and image catalogs from the v0.84.0 tag JSON output.
- Added Baseten provider catalog/auth parity:
  - provider id `baseten`
  - `BASETEN_API_KEY`
  - `openai-completions` runtime path
  - `zai-org/GLM-5.2`, `zai-org/GLM-5.2-Fast`, `moonshotai/Kimi-K2.6`, and related Baseten metadata from `scripts/generate-models.ts`.
- Added `Model.sampling_params` / `StreamOptions.sampling_params` and OpenAI-compatible request merging:
  - model defaults merge with request params
  - request keys override model keys
  - merged params are applied last in OpenAI Completions and OpenAI/Azure Responses payloads so arbitrary sampling keys can override named request fields.
- Added Baseten `thinkingFormat: "baseten"` handling with configurable `chat_template_args` and optional `reasoning_effort`.
- Added vLLM `thinking_token_budget` support for OpenAI-compatible Completions when `supportsThinkingTokenBudget` is set, including the upstream `MIN_ANSWER_TOKENS = 1024` edge behavior.
- Added `supportsFinishReason: false` OpenAI-compatible stream inference so streams without provider finish reasons infer `stop` vs `toolUse` instead of failing.
- Fixed validation union coercion to preserve values that already match nullable `anyOf`/`oneOf` arms before coercing through earlier primitive arms.
- Changed generated text registry construction to append in small chunks, avoiding test-stack overflow with the larger v0.84.0 catalog.

### Named Rust evidence in this slice

`src/tests/release/v0840_release_test.rs`:

- `sampling_params_merge_and_override_openai_compatible_payloads`
- `baseten_catalog_and_reasoning_payload_match_v0840`
- `vllm_thinking_token_budget_edge_matrix`
- `nullable_anyof_oneof_preserves_matching_null_before_coercion`
- `supports_finish_reason_false_infers_terminal_stop_or_tool_use`

### Slice verification

Executed from `/workspace/projects/rs-ai`:

```bash
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json   python3 scripts/compare_upstream_registry_pairs.py   /workspace/tmp/pi-v0840   a5f43bf8aff3c55752432655f7334e3dafd1e256
cargo build
cargo test v0840_release_test -- --nocapture
cargo clippy --all-targets -- -D warnings
```

Results:

- Comparator: text `1153/1153`, image `42/42`, missing `0`, extra `0`
- `cargo build`: passed
- `cargo test v0840_release_test -- --nocapture`: `5 passed; 0 failed`
- `cargo clippy --all-targets -- -D warnings`: passed


### Catalog correction: release-pinned provider shards supersede dynamic 1212 evidence

Auditor correction accepted: the earlier `1212` text-pair evidence was generated from a fresh dynamic `models.dev`/OpenRouter aggregate and included 59 OpenRouter `:batch` aliases that are not present in either authoritative official-release artifact. It is superseded and must not be used as v0.84.0 parity evidence.

Authoritative catalog source for this release audit is offline and release-pinned:

- upstream tag worktree: `/workspace/tmp/pi-v0840` at `a5f43bf8aff3c55752432655f7334e3dafd1e256`
- unpacked npm package: `/workspace/tmp/pi-ai-0.84.0-package/package`
- provider shards: `/workspace/tmp/pi-ai-0.84.0-package/package/dist/providers/data/*.json`
- provider manifest `schemaVersion=3`, `generatedAt=2026-08-06T11:03:30.465Z`, `structureHash=3ed4153a80db1d4458c5d275334d995f90f9a9c3d77970a0262c715df0e43e79`
- extracted release JSON: `/workspace/tmp/pi-v0840-release-json/models.json`
- extractor metadata: `/workspace/tmp/pi-v0840-release-json/source-metadata.json`

Hardened extractor: `scripts/extract_release_model_shards.py` reads only local npm `dist/providers/data` shards, records package/shard/manifest hashes, verifies the tag worktree SHA when provided, and fails if any `:batch` aliases appear in the release shards. Current release-pinned text catalog is **1153 provider/id pairs across 38 providers and 9 APIs** with **0** `:batch` aliases; image catalog remains **42** pairs. Regression evidence is `release_pinned_catalog_has_no_unpinned_batch_aliases` in `src/tests/release/v0840_release_test.rs`.

### Committed slice 2: public deferred/background response lifecycle

Auditor priority correction: upstream commit `382aa641cc4c197dfa95ed684f187b5a39bc30ce` (`DRAFT: add openai background mode responses`) is public required behavior, not N/A and not deferred-tool-only.

Ported/adapted in this slice:

- Added public `DeferredHandle` and `DeferredRequest` Rust types.
- Added `StopReason::Deferred` and `Message.deferred` serde-compatible state.
- Added `StreamOptions.deferred` and `StreamOptions.wait` request fields.
- Extended `ApiProvider` with public `fetch_deferred` and `cancel_deferred` capability methods.
- Added top-level `registry::fetch_deferred` / `registry::cancel_deferred` dispatch.
- Extended `FauxProvider` with deterministic deferred/background lifecycle state:
  - submit with deferred option returns a handle and `StopReason::Deferred`
  - first N polls can return pending/deferred
  - ready poll streams the stored final assistant message
  - cancellation records handles and turns later fetches into in-band error messages
  - unknown handle fetches return in-band assistant errors.
- Added type aliases for boxed event streams and async cancellation futures to keep the public trait Send/pin-safe and Clippy-clean.

Named Rust evidence:

- `providers_upstream_test::tests::faux_provider_submits_polls_and_redeems_deferred_responses`
- `providers_upstream_test::tests::faux_provider_records_cancellation_and_fetches_cancelled_handle_as_error`
- `providers_upstream_test::tests::unsupported_deferred_capability_reports_in_band_provider_errors`
- Existing `provider::faux` stream tests remain active against the expanded provider.

Slice 2 verification:

```bash
cargo test providers_upstream_test -- --nocapture
cargo test provider::faux -- --nocapture
cargo test v0840_release_test -- --nocapture
cargo test
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json   python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0840 a5f43bf8aff3c55752432655f7334e3dafd1e256
cargo build
cargo clippy --all-targets -- -D warnings
```

Results: provider/deferred targeted tests passed; full `cargo test` passed with `836 passed; 0 failed`; doctest `1 passed`; comparator remains text `1153/1153`, image `42/42`, missing `0`, extra `0`; build/fmt/clippy passed.

### Committed slice 4: provider stream/error regressions

Ported/adapted executable provider-specific v0.84.0 regressions:

- Anthropic Messages preserves initial `content_block_start` text, thinking text, and thinking signature before later deltas.
- OpenAI/Azure Responses terminal handling now records `incomplete_details.reason` in `raw_stop_reason` as `status.reason`, maps only `incomplete.max_output_tokens` to `length`, and surfaces other incomplete reasons as errors.
- Google history conversion now requires tool-call IDs for Gemini 3+ models and preserves same-model signed empty text/thinking blocks instead of dropping the reasoning signatures.
- Bedrock failures now attach structured `bedrock_response_failure` diagnostics with status/errorCode/requestId where available while preserving the retry-facing error message.

Named Rust evidence:

- `anthropic_sse_parsing_test::tests::preserves_initial_text_and_thinking_from_content_block_start`
- `openai_responses_terminal_event_test::tests::finalizes_incomplete_max_output_terminal_events_as_length_stops`
- `openai_responses_terminal_event_test::tests::incomplete_non_max_output_reason_is_error_with_raw_reason`
- `google_gemini3_unsigned_tool_call_test::tests::no_skip_validator_for_unsigned_google_gen_ai_tool_calls` (updated to assert Gemini 3 IDs)
- `google_signed_empty_blocks_test::tests::preserves_same_model_empty_text_and_thinking_blocks_when_signed`
- `google_signed_empty_blocks_test::tests::drops_cross_model_empty_signed_thinking_because_signature_is_unusable`
- `bedrock_error_metadata_test::tests::bedrock_failure_diagnostic_preserves_status_code_and_request_id_without_rewriting_error_message`
- `bedrock_error_metadata_test::tests::bedrock_failure_diagnostic_drops_empty_or_overlong_values`

Slice verification:

```bash
cargo test bedrock_error_metadata_test -- --nocapture
cargo test anthropic_sse_parsing_test -- --nocapture
cargo test openai_responses_terminal_event_test -- --nocapture
cargo test google_gemini3_unsigned_tool_call_test -- --nocapture
cargo test google_signed_empty_blocks_test -- --nocapture
cargo test google_shared_convert_tools_test -- --nocapture
cargo test google_thinking_signature_test -- --nocapture
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json   python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0840 a5f43bf8aff3c55752432655f7334e3dafd1e256
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Results: targeted provider tests passed; full `cargo test` passed with `843 passed; 0 failed`; doctest `1 passed`; comparator remains text `1153/1153`, image `42/42`, missing `0`, extra `0`; build/fmt/clippy passed.

### Committed slice 5: runtime auth/options/telemetry semantics

Ported/adapted runtime/OAuth/telemetry v0.84.0 behavior:

- Added `merge_provider_headers` with case-insensitive null/deletion semantics for provider-resolved headers, preserving Rust request headers as concrete strings while still testing the upstream `ProviderHeaders` deletion model.
- Added cancellation-aware OAuth refresh seam (`refresh_with_cancel`) and `AuthResolutionOverrides.cancel`; refresh receives a cancellation receiver and preserves the rotated credential only after successful refresh.
- Added `RefreshOptions.providers` filtering so selected refreshes skip unrequested dynamic providers and ignore unknown provider IDs.
- Added `TelemetryContext` on `StreamOptions` and `ImagesOptions`, plus FauxProvider capture to prove telemetry metadata flows through normal stream and deferred fetch options; image options carry the same opaque telemetry context structurally.

Named Rust evidence:

- `auth::tests::merge_provider_headers_supports_null_deletion_case_insensitively`
- `auth::tests::resolve_oauth_refresh_receives_cancellation_signal_and_persists_success`
- `models_runtime_refresh_test::tests::refresh_provider_filter_skips_unrequested_dynamic_providers`
- `providers_upstream_test::tests::telemetry_context_flows_through_stream_and_deferred_fetch_options`

Slice verification:

```bash
cargo test providers_upstream_test -- --nocapture
cargo test auth::tests::merge_provider_headers_supports_null_deletion_case_insensitively -- --nocapture
cargo test auth::tests::resolve_oauth_refresh_receives_cancellation_signal_and_persists_success -- --nocapture
cargo test models_runtime_refresh_test::tests::refresh_provider_filter_skips_unrequested_dynamic_providers -- --nocapture
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json   python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0840 a5f43bf8aff3c55752432655f7334e3dafd1e256
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Results: targeted runtime/OAuth/telemetry tests passed; full `cargo test` passed with `847 passed; 0 failed`; doctest `1 passed`; comparator remains text `1153/1153`, image `42/42`, missing `0`, extra `0`; build/fmt/clippy passed.

### Closure slice 6: runtime/OAuth/telemetry dispatch, Codex WS account cache, Bedrock metadata

This slice closes the auditor-requested runtime/auth/telemetry cluster with executable provider/path coverage rather than helper-only assertions:

- Caller-owned OAuth refresh cancellation is wired through concrete OAuth providers (`AnthropicOAuth`, `CodexOAuth`, `KimiCodeOAuth`, `XaiOAuth`, `RadiusOAuth`, `OpenRouterOAuth`) with pre-cancel and mid-refresh tests. Aborted refreshes return typed `AbortError`-bearing OAuth errors and do not persist rotated credentials.
- `TelemetryContext` is captured through `registry::stream_simple`, deferred submit/fetch/cancel, and image provider dispatch.
- `ProviderHeaders` null/deletion semantics now flow through `merge_auth_into_request` into the real OpenAI-compatible request builder; explicit request headers still win afterward.
- `RefreshOptions.providers`, cancellation and supersession are covered through `ModelsRuntime::refresh`: aborted callers stop waiting on non-cooperative providers, and late first-generation refreshes cannot overwrite newer dynamic catalog state.
- Codex sticky WebSocket fallback is keyed by ChatGPT account id plus session id; one account’s WS failure no longer poisons another account using the same session id, while the original account reuses SSE.
- Bedrock failure diagnostics now extract SDK raw-response status and request id where available, suppress `Unknown`/transport names, keep retry-facing error messages untouched, and cover modeled/unmodeled send and stream diagnostic shapes.

Named Rust evidence:

- `auth_providers::tests::real_oauth_providers_pre_cancel_without_network_or_rotation`
- `auth_providers::tests::real_oauth_providers_mid_refresh_cancel_without_rotation`
- `auth_providers::tests::openrouter_oauth_honors_pre_cancel_without_mutation`
- `providers_upstream_test::tests::telemetry_context_flows_through_stream_simple_deferred_cancel_and_images`
- `models_runtime_auth_test::tests::provider_header_null_deletion_reaches_openai_request_builder`
- `models_runtime_refresh_test::tests::refresh_abort_stops_waiting_on_non_cooperative_provider`
- `models_runtime_refresh_test::tests::late_refresh_publication_is_rejected_after_supersession`
- `codex_ws_account_cache_test::tests::websocket_fallback_is_scoped_by_account_and_session`
- `bedrock_error_metadata_test::tests::*` (7-case status/requestId/code/suppression matrix)

### Closure slice 7: Google shared retry final gap

This slice ports upstream `google-shared.ts` / `google-shared-retry.test.ts` onto rs-ai's real Google REST stream call site:

- `stream_google` now uses a Google-specific provider retry config: no retries when `max_retries` is unset, explicit retry budget when set, and provider-style 500ms first backoff.
- Retryable headers-less Google SDK status semantics are covered through actual HTTP responses with no retry headers.
- `StreamOptions.cancel` is passed into the retry loop, so retry backoff honors caller cancellation using the existing Rust retry primitive.
- `max_retry_delay_ms` and `retry-after-ms` delay caps are covered.

Named Rust evidence:

- `google_shared_retry_test::tests::retries_headers_less_google_status_429_once_when_max_retries_is_one`
- `google_shared_retry_test::tests::does_not_retry_google_429_when_max_retries_is_unset`
- `google_shared_retry_test::tests::does_not_retry_google_non_retryable_status_400_even_with_budget`
- `google_shared_retry_test::tests::google_retry_config_defaults_to_500ms_backoff_and_honors_delay_cap`
- `google_shared_retry_test::tests::google_retry_after_delay_cap_fails_without_second_attempt`
- `google_shared_retry_test::tests::google_retry_backoff_honors_caller_cancellation`

### Final v0.84.0 completion status

The deterministic changed assertions called out in `docs/v0840-manifests.md` are now either ported/adapted with named Rust evidence, covered by existing deterministic provider/runtime tests, or explicitly N/A for live credentials/interactive UI/JS-runtime-only surfaces. The whole-corpus `packages/ai/test/*.test.ts` crosswalk is recorded in `docs/v0840-127-test-crosswalk.md`: **127/127** upstream test filenames accounted, including the 8 previously absent filenames (`cloudflare-stream`, `image-model-data`, `model-data-validation`, `openrouter-cache-control-models`, `provider-retry`, `reasoning-options`, `uuid`, `xai-responses`). No stale pending runtime/OAuth/telemetry/Bedrock/Codex/Anthropic/Responses/Google retry entries remain for the bounded v0.84.0 release audit.

Explicit rubric dispositions:

- `message_update` delta-only JSON/RPC change — **N/A** for rs-ai. It is coding-agent transport/serialization outside the `packages/ai` API/runtime scope and outside rs-ai's typed in-process `Message`/`Event` API; there is no JSON/RPC delta transport surface to port without inventing a parallel protocol.
- `ModelsStreamTransforms` → `ModelsRequestTransforms` — **ADAPTED**. rs-ai applies the equivalent request transforms across stream/simple/deferred fetch+cancel, not only streaming: auth/header transforms flow through `merge_auth_into_request` into provider request builders (`src/auth.rs`, `src/tests/auth/oauth/models_runtime_auth_test.rs::provider_header_null_deletion_reaches_openai_request_builder`), telemetry flows through `registry::stream_simple`, `registry::fetch_deferred`, `registry::cancel_deferred`, and image dispatch (`src/registry.rs`, `src/tests/transports/providers_upstream_test.rs::telemetry_context_flows_through_stream_simple_deferred_cancel_and_images`), and deferred public dispatch is represented by `ApiProvider::fetch_deferred` / `ApiProvider::cancel_deferred` plus the registry wrappers.

This file is the release-audit ledger for `rs-ai`. It must be updated in the same commit as every future upstream `@earendil-works/pi-ai` release audit.

## Historical prior release: v0.83.0

- Upstream package: `@earendil-works/pi-ai`
- Historical accepted release: `v0.83.0`
- Upstream tag/commit: `845d6ff1f6643aba440341cce877ce1c43ebbc39`
- Previous accepted upstream baseline: `v0.82.1` / `b4f293684bba718d59cc1157679bcf6157b3a7f5`
- Audited range: `b4f293684bba718d59cc1157679bcf6157b3a7f5..845d6ff1f6643aba440341cce877ce1c43ebbc39`
- Scope: `packages/ai` only; no newer-main chase.

## Exact upstream change set

The v0.83.0 release changes 41 `packages/ai` paths:

- Release/docs/package metadata:
  - `CHANGELOG.md`
  - `README.md`
  - `package.json`
- Catalog generation:
  - `scripts/generate-models.ts`
- API/runtime:
  - `src/api/anthropic-messages.ts`
  - `src/api/azure-openai-responses.ts`
  - `src/api/bedrock-converse-stream.ts`
  - `src/api/google-generative-ai.ts`
  - `src/api/google-vertex.ts`
  - `src/api/mistral-conversations.ts`
  - `src/api/openai-codex-responses.ts`
  - `src/api/openai-completions.ts`
  - `src/api/openai-responses-shared.ts`
  - `src/api/openai-responses.ts`
  - `src/api/openrouter-images.ts`
  - `src/api/pi-messages.ts`
  - `src/api/simple-options.ts`
- Auth/provider/runtime support:
  - `src/auth/oauth/openrouter.ts`
  - `src/auth/resolve.ts`
  - `src/providers/faux.ts`
  - `src/types.ts`
- Tests:
  - `test/anthropic-sse-parsing.test.ts`
  - `test/azure-openai-responses-reasoning-replay.test.ts`
  - `test/bedrock-credentials.test.ts`
  - `test/bedrock-raw-stop-reason.test.ts`
  - `test/constrained-sampling.test.ts`
  - `test/faux-provider.test.ts`
  - `test/fetch-option.test.ts`
  - `test/github-copilot-anthropic.test.ts`
  - `test/google-raw-stop-reason.test.ts`
  - `test/mistral-raw-stop-reason.test.ts`
  - `test/models-runtime.test.ts`
  - `test/oauth-auth.test.ts`
  - `test/openai-completions-raw-stop-reason.test.ts`
  - `test/openai-completions-tool-choice.test.ts`
  - `test/openai-responses-partial-json-cleanup.test.ts`
  - `test/openai-responses-terminal-event.test.ts`
  - `test/openrouter-oauth.test.ts`
  - `test/pi-messages.test.ts`
  - `test/qwen-token-plan-models.test.ts`
  - `test/validation.test.ts`

## Rust implementation and disposition

### Catalog and metadata

- Regenerated `src/models_generated.rs` from hydrated v0.83.0 JSON shards.
- Text provider/id comparator:
  - `upstream=1153 local=1153 missing=0 extra=0`
- Image provider/id comparator:
  - `upstream=40 local=40 missing=0 extra=0`
- Repro command:

```bash
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0830-json \
  scripts/compare_upstream_registry_pairs.py \
  /workspace/tmp/pi-v0830 \
  845d6ff1f6643aba440341cce877ce1c43ebbc39
```

### Stop reason and raw stop reason

- Added `StopReason::Pending` to mirror the v0.83 public type surface.
- Added `Message.raw_stop_reason` with serde-compatible camelCase field handling.
- Streaming assistant partials now start as `Pending` instead of absent/successful stop state.
- `Pending` must not emit a successful `Done`; streams that end pending are surfaced as errors.
- Raw stop reason capture is wired for:
  - OpenAI Completions
  - OpenAI Responses / Azure OpenAI Responses
  - OpenAI Codex Responses
  - Anthropic Messages
  - Google Generative AI / Vertex shared paths
  - Mistral Conversations
  - Bedrock Converse Stream
- Tests include raw-stop and pending/no-terminal behavior in provider fixtures and `src/tests/release/v0830_release_test.rs`.

### Constrained sampling

Already completed as part of the v0.82.0 corrective work and retained for v0.83.0:

- `Tool.constrained_sampling` with camelCase serde.
- JSON-schema strict constrained sampling.
- Grammar constrained sampling:
  - lark/regex variant resolution
  - one-required-string schema validation
  - custom-tool request shape
  - monotonic streamed JSON delta reconstruction
- Integrated request-shape handling for OpenAI Completions, Responses/Azure, and Codex payloads.
- Tests are in `src/tests/release/v0830_release_test.rs` and provider request fixtures.

### Retry behavior

Already completed as part of v0.82.0 corrective work and retained:

- `do_with_retry` honors `x-should-retry`.
- Excessive provider-requested retry delays fail immediately instead of silently clamping.
- Backoff sleep can be aborted via `do_with_retry_cancel`.
- Transport/no-status request errors are retryable.
- Tests cover direct retry helper behavior and actual OpenAI provider request path.

### OAuth/auth/model runtime

Existing v0.82.1 work remains active:

- `ModelsError::with_cause` preserves underlying cause detail in surfaced messages.
- Production auth resolution uses cause-preserving errors for API-key and OAuth failure paths.
- Radius OAuth discovery-only gateway routing is supported.
- Radius dynamic catalog refresh supports ETag/`If-None-Match` and 304 cached reuse.
- Anthropic `ANTHROPIC_AUTH_TOKEN` participates in env discovery and is sent as `Authorization: Bearer`, not `x-api-key`.

### v0.83 corrective runtime details

- Malformed OpenAI tool delta regression `malformed_openai_delta_preserves_function_when_custom_is_empty` verifies valid `function` payload plus empty `custom` preserves function name and arguments.
- Added focused raw/pending parser tests for `anthropic_raw_stop_and_missing_stop_are_executable`, `responses_pending_status_is_error_and_preserves_raw_stop_reason`, `azure_responses_pending_terminal_status_is_error_with_raw_reason`, `codex_pending_status_is_error_and_raw`, `google_and_mistral_raw_stop_reasons_are_executable`, and `bedrock_raw_stop_reason_helper_errors_unknown_and_preserves_raw`.

- OAuth resolution now refreshes tokens early by default when less than 5 minutes of validity remain. `min_oauth_validity_ms` is floored to that upstream default 5-minute window, and refreshed credentials are rejected when they still do not satisfy the effective minimum validity.
- OpenAI malformed/empty custom and grammar constrained-sampling request/stream behavior remains covered by v0.82/v0.83 release fixtures.
- Provider raw stop reason tests cover OpenAI plus provider-specific raw/pending behavior through release and provider tests.
- Bedrock explicit profile precedence is tested: an explicit `StreamOptions.profile`/profile seam suppresses standard-endpoint region pinning even when ambient access keys exist, while ARN regions and custom endpoints retain priority.

### Fetch option changes

- Upstream v0.83.0 adds optional custom fetch plumbing in TypeScript APIs.
- Rust uses `reqwest` clients and proxy-aware builders instead of JavaScript `fetch`; there is no direct `fetch` injection point.
- Disposition: **N/A for Rust runtime**, with existing request/proxy/client tests covering the equivalent Rust transport surface.

### TypeScript-only/model-catalog tests

- Type-only/export/build-surface changes are not applicable to Rust.
- Disposition: **N/A**, with Rust model-data structural validation and serde model shape tests covering runtime-valid metadata.

### Live/credential tests

- Upstream live credential/e2e matrix updates remain N/A when they require external provider credentials or nondeterministic live service behavior.
- Deterministic request, stream, retry, error, and metadata behavior is covered by local mock-server tests.

## Verification for current release

Executed from `/workspace/projects/rs-ai`:

```bash
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0830-json \
  scripts/compare_upstream_registry_pairs.py \
  /workspace/tmp/pi-v0830 \
  845d6ff1f6643aba440341cce877ce1c43ebbc39
cargo build
cargo test   # run 3 times
cargo clippy --all-targets -- -D warnings
```

Results:

- Comparator: text `1153/1153`, image `40/40`, missing `0`, extra `0`
- `cargo build`: passed
- `cargo test` x3: each run `822 passed; 0 failed; 0 ignored`; doctest `1 passed; 0 failed`
- `cargo clippy --all-targets -- -D warnings`: passed

## Release-audit policy

For every future upstream `@earendil-works/pi-ai` release audit:

1. Pin the exact previous accepted upstream tag/SHA and new upstream tag/SHA.
2. Do not chase upstream main beyond the requested release tag.
3. List the exact changed `packages/ai` paths or a grouped matrix that accounts for all paths by count.
4. Regenerate and compare model/image catalogs with the JSON-shard-aware comparator when applicable.
5. Implement all applicable runtime/API/provider/model/tool/stream/error/usage behavior in production Rust paths.
6. Document every N/A decision with the concrete reason.
7. Run the required Rust gates.
8. Update this `RELEASE.md` in the same release commit before reporting completion.
