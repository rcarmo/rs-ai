# pi-durable v1.0.1 native R1a crosswalk

Source: fixed official `@earendil-works/pi-durable@1.0.1` tree `a7229ddc21810d6245105978033b7df645ecc2f7`; verified artifact SHA-256 `c4bc1ea49653ee972045a5de755756b633e3be0e1528ae32f5f32d82e012db7d`. The v1.0.0→v1.0.1 delta changes only package metadata and changelog; the 60 source files, 42 test suites and specification are unchanged.

R1a implements atomic storage/session semantics. R1b implements persistent root no-tool generation. R1c implements the useful model→owned tool→result-aware answer vertical with exact-version safe replay, bottom-up abort and non-aborting close drain. `ADAPTED` names a tested native invariant; `LATER` names remaining full-contract work. No row claims complete parity.

## Official 60-source disposition

| Official source | Status | R1a disposition |
|---|---|---|
| `packages/durable/src/documents.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/entries.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/env/index.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/env/node.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/errors.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/agent.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/compaction.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/context.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/define.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/events.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/generation.ts` | ADAPTED | `model.rs` and `harness.rs`: pinned intent-before-effect, owned registry stream, terminal/usage atomic settlement and explicit resume. |
| `packages/durable/src/harness/harness.ts` | ADAPTED | Public root `DurableHarness`, FIFO executor, admission fence, inspect/context/wait and common close drain. |
| `packages/durable/src/harness/inbox.ts` | ADAPTED | R1b `pi.inbox` is an authoritative bounded pending-submission snapshot; steer/follow-up modes and withdrawal remain later. |
| `packages/durable/src/harness/json.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/live.ts` | ADAPTED | R1b `pi.live` tracks the current queued/running/terminal root generation across settlement and reopen. |
| `packages/durable/src/harness/output.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/prompt.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/registry.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/scheduler.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/submissions.ts` | ADAPTED | `submission.rs`: typed request conflict, idempotent winner reacquisition and detached committed views. |
| `packages/durable/src/harness/task-graph.ts` | ADAPTED | R1c generation owns tool children, remains `completing` until all settle, and aborts/drains bottom-up; generic graphs remain later. |
| `packages/durable/src/harness/tool.ts` | ADAPTED | R1c durable offered schema, intent-before-effect, owned child task, bounded result and final answer continuation. |
| `packages/durable/src/harness/types.ts` | ADAPTED | R1a records plus R1b pinned model/options/usage/content and public submission DTOs. |
| `packages/durable/src/harness/usage.ts` | ADAPTED | R1b atomically stores complete per-turn usage/cost plus checked aggregate totals across journal reopen. |
| `packages/durable/src/harness/util.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/harness/view.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/ids.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/index.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/session/forks.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/session/observation.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/session/session.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/session/transaction.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/storage/jsonl/index.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/storage/jsonl/node.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/storage/jsonl/storage.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/storage/memory.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/storage/sqlite/database.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/storage/sqlite/index.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/storage/sqlite/migrations.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/storage/sqlite/node.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/storage/sqlite/storage.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/tasks.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/testing/assertions.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/testing/index.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/testing/runner.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/testing/storage-benchmark.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/testing/storage-conformance.ts` | ADAPTED | `src/tests/durable/storage_conformance_test.rs` runs the same atomic/detached/high-water contract against memory and journal backends. |
| `packages/durable/src/testing/types.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |
| `packages/durable/src/tools/bash.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/edit-diff.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/edit.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/env.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/file-mutation-queue.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/image.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/index.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/path-utils.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/read.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/tools/write.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/truncate.ts` | LATER | Outside R1a foundation; retained for R1b/R1c or later full-contract work. |
| `packages/durable/src/types.ts` | ADAPTED | Atomic records/session/storage invariant adapted in native kernel. |

## Official 42-suite disposition

| Official suite | Status | R1a disposition |
|---|---|---|
| `packages/durable/test/chord-guide.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/env-node-spill.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/env-node.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/env-truncate.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-compaction.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-context.test.ts` | ADAPTED | Detached committed root context and passive-write busy fencing are covered in R1b lifecycle/submission tests. |
| `packages/durable/test/harness-conversations.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-events.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-generation-recovery.test.ts` | ADAPTED | Persistent running→pending open reconciliation, zero-effect open and explicit resume proof. |
| `packages/durable/test/harness-generation.test.ts` | ADAPTED | Production local HTTP/SSE registry proof, pinned behaviour, terminal validation and usage settlement. |
| `packages/durable/test/harness-inbox.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-inspect.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-lifecycle.test.ts` | ADAPTED | Caller-drop retention, durable bottom-up abort, non-cooperative tool drain, close-without-abort and successor fencing. |
| `packages/durable/test/harness-live-deltas.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-output.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-ownership.test.ts` | ADAPTED | FIFO model phases plus R1c owned tool executors, completing drain, caller-drop retention and phase fences. |
| `packages/durable/test/harness-prompt.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-registry.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-structured.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-submissions.test.ts` | ADAPTED | Request-ID reacquisition/conflict, passive writes, failure usage and strict submission bounds. |
| `packages/durable/test/harness-task-graph.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-tasks-recovery.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-tasks.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/harness-tools-recovery.test.ts` | ADAPTED | Exact safe implementation/version/schema replay, stable idempotency key and conservative unsafe/changed/missing settlement. |
| `packages/durable/test/harness-tools.test.ts` | ADAPTED | Real model→tool→answer, original versus validated execution arguments, schema/limit failures and atomic tool usage/outcome. |
| `packages/durable/test/harness-view.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/jsonl-storage.test.ts` | ADAPTED | `journal_recovery_test.rs`: framed reopen, every final-frame byte cut, malformed complete data and append/sync/ack fault truth. |
| `packages/durable/test/memory-storage.test.ts` | ADAPTED | `storage_conformance_test.rs::memory_storage_passes_atomic_detached_conformance`. |
| `packages/durable/test/session-checkpoints-migrations.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/session-definitions.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/session-documents.test.ts` | ADAPTED | R1a atomic document record and encoded-size coverage in shared storage conformance; document APIs remain later. |
| `packages/durable/test/session-forks.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/session-states.test.ts` | ADAPTED | R1a transition, terminal immutability, owner/reference and close fencing proofs; full session states remain later. |
| `packages/durable/test/session-tables.test.ts` | ADAPTED | R1a mixed commit contains entries/tasks/submissions/documents and durable counters; migrations remain later. |
| `packages/durable/test/session-watches.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/spec-usage.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/sqlite-facade.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/sqlite-migrations.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/sqlite-storage.test.ts` | LATER | SQLite backend is later; its backend-neutral atomicity requirements are adapted through shared conformance only. |
| `packages/durable/test/storage-runtime-boundary.test.ts` | ADAPTED | Writer claims, in-process path ownership, poison-until-reopen and owned storage worker boundaries are tested in R1a. |
| `packages/durable/test/tools.test.ts` | LATER | Outside R1a foundation; retained for later implementation and evidence. |
| `packages/durable/test/types.test.ts` | ADAPTED | Bounded custom ID deserialization, shared ID namespace, references, JSON and commit limits are covered in storage conformance. |

## R1a native evidence targets

- Atomic mixed commits for entries, complete tasks, submissions, documents and high-water counters.
- Detached serde JSON ownership and fail-closed limits/ranges.
- Bounded session and storage queues with pre-admission caller-drop tombstones and owned non-cancellable post-admission settlement.
- Memory/reference and persistent framed-journal conformance.
- Reopen, partial-final-tail handling, corruption rejection, uncertainty poison, close fencing and public visibility after adoption.

## Final R1b local evidence

The exact 11-path candidate passed 38 focused durable tests, 988 no-default tests and 1,145 all-feature tests with zero failures or ignored tests. Strict Clippy passed for no-default and all-feature profiles. Security, SBOM, licence, vulnerability, v1.0.1 manifest, metadata, reproducibility, hydration and fault/self-test gates passed. A genuine detached Git worktree from the R1a checkpoint passed exact `make check`. The isolated Rust 1.85 module check validates syntax and the durable contract shape; the unchanged whole dependency graph retains its documented Rust 1.88 floor.

## Explicit later work

R1b/R1c must add public model and executable-tool APIs, generation→tool ownership, replay policy, abort/close effect draining and root-conversation answer recovery before any same-version v1.0.1 durable retag. Generic documents/tasks, forks, inbox, watches/events, extensions, partial output, deferred work, compaction, subagents, remote storage and SQLite remain later. R1a alone is not a useful assistant harness and is not pi-durable completion.
