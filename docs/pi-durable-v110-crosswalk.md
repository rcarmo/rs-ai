# pi-durable v1.1.0 upgrade crosswalk

Status: **IN PROGRESS — NOT ACCEPTED OR PUBLISHED**.

Official range: `a7229ddc21810d6245105978033b7df645ecc2f7..abe508e1b89912adde45528136c3221eb69acdd7` (v1.0.1 → v1.1.0). Pinned npm package: `@earendil-works/pi-durable@1.1.0`, SHA-256 `a0f95b4a418e8bc219e9cbde06baedada940c62c47068829208e4fff278c07be`. Registry SHA-512 integrity matches; provenance signature/transparency has not been independently verified. npm metadata omits gitHead; the official tag supplies the source bound.

Inventory: 59 changed paths; 67 source paths; 49 executable test paths; 90 total test/fixture/support paths. Exact Git shortstat: 59 files changed, 5481 insertions(+), 361 deletions(-).

The previous R1 vertical is a partial baseline. Updating only package metadata or the changed upstream paths does not close existing documents/tasks/forks/events/extensions/output/deferred/compaction/subagents/environment/SQLite/cross-process gaps. The 4,747-line v1.1.0 specification and full source/test corpus form the acceptance scope.

## Changed-path disposition

| Status | Official path | Disposition | Native evidence |
|---|---|---|---|
| M | `packages/durable/CHANGELOG.md` | PENDING | Not yet accepted |
| M | `packages/durable/README.md` | PENDING | Not yet accepted |
| M | `packages/durable/docs/spec.md` | PENDING | Not yet accepted |
| M | `packages/durable/package.json` | PENDING | Not yet accepted |
| A | `packages/durable/src/env/decode.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/env/index.ts` | PENDING | Not yet accepted |
| A | `packages/durable/src/env/line-scan.ts` | PENDING | Not yet accepted |
| A | `packages/durable/src/env/node-watch.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/env/node.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/harness/agent.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/harness/compaction.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/harness/context.ts` | ADAPTED native tool-message derivation; cache/edit/fork gaps | New `src/durable/context.rs` derives native entries, repairs tool-result call order, synthesizes missing results and drops duplicate/orphan results. Tool-call assistant entries persist before effects; later submissions retain prior rounds, including after recovery. `message_context({ at })` is read-only and inclusive. Tests cover missing/order/duplicates, follow-up preservation, recovery no-dispatch and FIFO queued cutoff. Current-message range caching is bounded, timed, configurable/zeroable, extends after adoption, and rebuilds on heads/edits or invalidates changed task input; idle-only native heads/omit-replace edits are implemented with historical cutoff, latest-wins and leading-system rules. The newest head marker contributes first; older markers retain only in-range edits. Orphan-result removal precedes system-prefix promotion, and missing-result text matches upstream. Nine native context regressions include head/omit ordering, orphan/system/excluded replacement cases and detached ContextView entries/contributions. Session/harness `context_view` exposes resolved head, active entries, aligned contributions and repaired messages without dispatch. Generic EntryDraft context contributions are adapted in `src/durable/entries.rs`; Current-message appended ranges/raw open-result repair and native busy retention are adapted: nonterminal tasks pin ranges, adopted idle starts expiry, zero drops idle only and reads do not slide expiry. Busy-boundary updates/forks/historical and full-view range reuse/generation-task reuse are absent. |
| M | `packages/durable/src/harness/generation.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/harness/harness.ts` | ADAPTED provider identity and historical text-context subset; broader gaps | `DurableHarness::context_with_options(ContextOptions { at })` cuts inclusively at a visible entry, rejects missing/foreign entries and leaves state/effects unchanged. `context()` remains compatible. Submission regression tests passive/user/answer/tail cuts and no dispatch/mutation. `message_context` additionally reconstructs tool messages, native context edits and head resets, with newest-marker-first ordering and detached historical cuts. Native `context_view` exposes detached resolved head, active entries and aligned contributions. Generic passive EntryDraft append and direct entry reads are adapted; idle stale heads reject. Current-message appended-range reuse is adapted. Fork ancestry, flat upstream EntryRecord encoding and historical/full-view range reuse are unimplemented. |
| M | `packages/durable/src/harness/output.ts` | PENDING | Not yet accepted |
| A | `packages/durable/src/harness/provider.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/harness/scheduler.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/harness/tool.ts` | ADAPTED model access/timing subset; output/environment gaps | `ToolExecution.models` receives the same `Arc<dyn DurableModels>` in normal/recovered attempts. Host injection and default RegistryModels support completion/classification; tests verify pointer identity, nested call, recovered current host and HTTP completion/Decisions. `ToolExecution.entries` adapts passive append/read access scoped to nonterminal tool tasks; normal/recovered regressions verify attribution/detached records and retained-handle rejection after terminal settlement. Tool environment/progress-output windows, invocation-end fences and full task/hook APIs are absent. |
| M | `packages/durable/src/harness/types.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/harness/view.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/index.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/session/session.ts` | IMPLEMENTED injectable lifecycle clock delta | `DurableSession::open_with_clock` accepts `LifecycleClock`; the default constructor uses wall-clock milliseconds. `DurableHarness::open_with_tools_and_clock` forwards the host clock. Tests verify exact start/end stamps, no clock calls for unstamped pending work, preserved recovered starts, negative-time rejection and harness propagation. |
| M | `packages/durable/src/session/transaction.ts` | ADAPTED task lifecycle subset | Session commit admission stamps missing start/end values using the host clock and preserves existing starts; storage rejects changed starts, negative times and nonterminal end times. Native CommitBatch and entry-only `transact_entries` adapt transaction staging. Six transaction tests plus two lifecycle regressions cover read-before-write, same-batch references, rollback/panic, no-op reads, concurrent allocation, journal reopen, cancellation/close/uncertainty. Full generic task/document/conversation and async transaction APIs are incomplete. |
| M | `packages/durable/src/storage/memory.ts` | PENDING | Not yet accepted |
| A | `packages/durable/src/storage/scan.ts` | ADAPTED filtered session-query subset | `EntryQuery` adds inclusive ID bounds/kind; `TaskQuery` kind/state/abort; `SubmissionQuery` status. Session/harness paged reads run on the mutation queue without cloning the whole snapshot. Entry ranges seek in the BTreeMap; cursor/default direction and detached values are preserved. Session `tasks_in`/`submissions_in` permit optional conversation scope; two-conversation pagination/filter/isolation and poison/close regressions verify combined ID order. Tests cover filtering, read-after-admitted-commit, poison/close and harness results. Fork/background/conversation queries and SQLite are absent; native IDs use positive i64. |
| A | `packages/durable/src/storage/sqlite/cloudflare.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/storage/sqlite/storage.ts` | PENDING | Not yet accepted |
| A | `packages/durable/src/testing/env-conformance.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/testing/index.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/testing/runner.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/testing/storage-conformance.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/testing/types.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/tools/bash.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/tools/image.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/tools/index.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/tools/read.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/truncate.ts` | PENDING | Not yet accepted |
| M | `packages/durable/src/types.ts` | PENDING | Not yet accepted |
| A | `packages/durable/test/env-line-scan.test.ts` | PENDING | Not yet accepted |
| A | `packages/durable/test/env-node-conformance.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/env-node.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-compaction.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-context.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-conversations.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-generation-recovery.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-generation.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-inbox.test.ts` | PENDING | Not yet accepted |
| A | `packages/durable/test/harness-output-skip.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-output.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-registry.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-tasks-recovery.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-tasks.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-tools.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/harness-view.test.ts` | PENDING | Not yet accepted |
| A | `packages/durable/test/provider-session-cache-e2e.test.ts` | PENDING | Not yet accepted |
| A | `packages/durable/test/sqlite-cloudflare.test.ts` | PENDING | Not yet accepted |
| A | `packages/durable/test/system-order-cache-e2e.test.ts` | PENDING | Not yet accepted |
| A | `packages/durable/test/tools-read-differential.test.ts` | PENDING | Not yet accepted |
| M | `packages/durable/test/tools.test.ts` | PENDING | Not yet accepted |

## Full test and support corpus

| Official path | Disposition | Native evidence |
|---|---|---|
| `packages/durable/test/chat-support.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/chord-guide.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/env-line-scan.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/env-node-conformance.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/env-node-spill.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/env-node.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/env-truncate.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/examples/00-conversation.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/01-documents.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/02-forks.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/03-owned-conversations.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/04-chord-state.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/05-watches.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/06-harness.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/07-configuration.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/08-harness-conversations.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/09-context.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/10-registry-reload.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/11-extension-state.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/12-tasks.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/13-recovery.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/14-chat.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/15-system-prompt.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/16-real-model.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/17-coding-tools.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/18-print.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/19-json.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/20-inbox.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/21-late-join.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/22-subagent-foreground.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/23-subagent-background.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/24-child-tasks.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/25-compaction.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/26-coding-agent.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/27-plan-mode.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/28-reviewer.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/29-sandbox-per-conversation.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/30-tool-override.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/examples/31-reload-and-restart.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/fixtures/delete-buffer.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/fixtures/utf8-byte-length-without-buffer.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/harness-compaction.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-context.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-conversations.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-events.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-generation-recovery.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-generation.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-inbox.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-inspect.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-lifecycle.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-live-deltas.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-output-skip.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-output.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-ownership.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-prompt.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-registry.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-structured.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-submissions.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-support.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/harness-task-graph.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-tasks-recovery.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-tasks.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-tools-recovery.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-tools.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/harness-view.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/jsonl-storage.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/memory-storage.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/provider-session-cache-e2e.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/session-checkpoints-migrations.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/session-definitions.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/session-documents.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/session-forks.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/session-states.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/session-support.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/session-tables.test.ts` | ADAPTED generic append subset | Eight tests in `src/tests/durable/entries_test.rs` plus lifecycle cancellation/poison checks cover custom kinds, self heads, optional data/model, unique allocation, detached records, rejected writes and memory/journal context. `src/tests/durable/transaction_test.rs` adds six tests for task-scoped attribution and entry-only callback staging, ReadAfterWrite, rollback/panic, read-only no-op, serialized reads/allocation and journal reopen; two lifecycle tests cover cancellation/close/poison. Other table/transaction contracts are unreviewed. |
| `packages/durable/test/session-watches.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/spec-usage.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/sqlite-cloudflare.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/sqlite-facade.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/sqlite-migrations.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/sqlite-storage.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/storage-memory.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/storage-runtime-boundary.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/storage.bench.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/system-order-cache-e2e.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/task-support.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/tool-output-bench.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/durable/test/tools-read-differential.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/tools.test.ts` | PENDING | Not yet accepted |
| `packages/durable/test/types.test.ts` | PENDING | Not yet accepted |

## Full source corpus

| Official path | Disposition | Native evidence |
|---|---|---|
| `packages/durable/src/documents.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/entries.ts` | ADAPTED generic passive draft subset | `src/durable/entries.rs` accepts custom kinds, optional data/model, resolved self heads and edits; `src/tests/durable/entries_test.rs` covers wire omission/null, model-less entries, contributions and recovery. `EntryDefinition<D>` adapts unregistered kind tokens, required typed draft data, kind-only matching and Serde decoding. A regression covers equal-kind tokens, cloning without data Clone, malformed/missing/null data and no persistent definitions. Flat upstream record encoding is absent. |
| `packages/durable/src/env/decode.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/env/index.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/env/line-scan.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/env/node-watch.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/env/node.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/errors.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/agent.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/compaction.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/context.ts` | ADAPTED native reconstruction and head-order subset | `src/durable/context.rs` and nine tests in `src/tests/durable/context_test.rs` cover result repair, excluded assistants, marker-first ordering, historical cuts, in-range edits, system-prefix promotion and native ContextView entries/contributions. Session lifecycle checks cover admitted-commit ordering and poison/close. Current-message range regression verifies appended-only decoding, late tool results, heads/edits and full-derivation equivalence; session regressions cover unrelated/task-input commits, budget overflow and growing reads. Native busy retention is covered by zero/short-retention and non-sliding idle-deadline tests. Forks/background ownership, generic upstream record encoding, generation-task and historical/full-view range reuse remain open. |
| `packages/durable/src/harness/define.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/events.ts` | ADAPTED native adopted-state watch subset | `src/durable/events.rs` and four tests in `src/tests/durable/events_test.rs` cover atomic snapshot registration, adopted commits, overflow reconciliation, stop/drop slots and worker-unwind termination. Full AgentEvent, listener, acquisition abort and transient progress are absent. |
| `packages/durable/src/harness/generation.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/harness.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/inbox.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/json.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/live.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/output.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/prompt.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/provider.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/registry.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/scheduler.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/submissions.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/task-graph.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/tool.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/types.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/usage.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/util.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/harness/view.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/ids.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/index.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/session/forks.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/session/observation.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/session/session.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/session/transaction.ts` | ADAPTED native append and lifecycle subset | Session-line `append_entry` assigns IDs/seq, validates references, adopts/publishes after settlement and survives admitted caller drop. Eight generic tests plus two lifecycle regressions include ordered same-batch references, concurrent IDs, tombstoning, poison, journal reopen and no dispatch. Entry-only synchronous `transact_entries` stages a read-then-append batch with rollback and shared adoption/cancellation semantics. `transact_task_entries` adapts nonterminal same-conversation attribution with missing/terminal callback fencing. Conversation table and full task/document/async transaction semantics are absent. |
| `packages/durable/src/storage/jsonl/index.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/jsonl/node.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/jsonl/storage.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/memory.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/scan.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/sqlite/cloudflare.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/sqlite/database.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/sqlite/index.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/sqlite/migrations.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/sqlite/node.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/storage/sqlite/storage.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tasks.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/testing/assertions.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/testing/env-conformance.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/testing/index.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/testing/runner.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/testing/storage-benchmark.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/testing/storage-conformance.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/testing/types.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/bash.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/edit-diff.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/edit.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/env.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/file-mutation-queue.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/image.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/index.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/path-utils.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/read.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/tools/write.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/truncate.ts` | PENDING | No full-contract acceptance yet |
| `packages/durable/src/types.ts` | PENDING | No full-contract acceptance yet |
