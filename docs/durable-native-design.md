# Native durable R1a/R1b foundation

R1a adds the storage/session kernel. R1b adds persistent model generation through the existing rs-ai provider registry. R1c adds the useful model→owned tool→result-aware answer vertical.

## Implemented contract

- Atomic `CommitBatch` writes entries, complete tasks, submissions, request-ID mappings, documents and post-commit high-water counters.
- Non-zero IDs are restricted to `1..=i64::MAX`; allocation and payload limits fail before persistence.
- JSON values are serialised and detached at storage boundaries. Entry kinds are `user`, `assistant`, `tool_result` or `model_error`; task kinds are `generation` or `tool`; document kinds are `pi.agent`, `pi.live`, `pi.inbox`, `pi.usage` or `pi.checkpoint`. Task and document versions start at 1.
- `MemoryStorage` is the reference backend. `JournalStorage` is the persistent cycle-1 backend and uses no new dependency.
- One writer claim exists per storage instance. Journal paths are canonicalised and exclusively owned in-process; a second alias/open fails. Close drops the file and releases path ownership even if the storage value remains retained.
- The session queue is bounded to 64 commands; the storage queue is bounded to 16.
- Caller drop before dequeue is a tombstone with no effect. After dequeue/admission, write, acknowledgement, adoption/poison and publication settlement continue under owned workers.
- `close()` seals public admission, rejects unadmitted work, waits for admitted settlement, closes storage and joins both workers. It writes no durable user abort or terminal task result.
- An uncertain storage result poisons reads and later dispatch until reopen.
- Submission states are `pending`, `done`, `failed` or `aborted`. `pending` has neither answer nor reason; `done` has an answer and no reason; `failed`/`aborted` have a reason and no answer. Only `pending` can transition, and terminal records are immutable.

## R1b model generation

- `DurableHarness` owns root conversation 1, idempotent submissions, inspection, context reads, explicit resume and a single FIFO model executor. Wakeups are coalesced and cannot fill a bounded command queue.
- A submission atomically commits the user entry, complete generation task, request index and live document before any provider effect. Dropping the submit future does not cancel admission or scheduling.
- `PinnedModel` stores the complete non-secret model behaviour record. Base URL, headers and credentials stay process-local. Dispatch combines pinned behaviour with only the current registry endpoint/auth fields, so later catalogue changes do not alter persisted requests.
- `PinnedOptions`, static system prompt, context cutoff and logical attempt are durable. Deferred execution and provider retries are disabled. Retrying a persisted intent is at-least-once for billing and effects.
- `RegistryModelRunner` calls the existing exported rs-ai registry stream. It accepts one `Stop`/`Length` terminal, preserves bounded text/thinking content and complete validated usage/cost fields, and rejects missing/duplicate/malformed terminals.
- Tool calls settle as `durable_tool_unsupported_r1b`. Unexpected deferred handles receive one best-effort cancellation and settle as `durable_deferred_unsupported`; R1b never polls them.
- Running tasks reconcile to pending on open without dispatch and retain their prepared context. Newly queued tasks prepare context only at provider-phase admission, after prior settlement; passive entries and prior answers are ordered before the target user input. Explicit `resume()` schedules recovered work. Terminal submissions perform no effect after reopen.
- Model outcome, assistant entry, authoritative pinned model identity, full per-turn usage/cost fields, checked aggregate totals, terminal task and terminal submission commit in one batch. `pi.agent`, `pi.live`, `pi.inbox` and `pi.usage` are detached built-in snapshots; inbox/live remain authoritative while follow-ups queue and across reopen.
- `close()` seals public admission and linearises against the provider-phase gate. Already admitted providers drain and settle; selected or queued pending tasks perform no new effect and remain resumable after reopen. Close always joins the executor and closes storage, including executor failure paths. It writes no abort or false terminal result. Wait cancellation cancels only that waiter.
- R1b persists terminal output only; provider partials are consumed by the owned executor and are not exposed or stored.
- Local evidence for the final R1b candidate includes 38 focused durable tests, 988 no-default tests, 1,145 all-feature tests, strict Clippy for both feature profiles, security/catalog validators, hydration/fault self-tests and a genuine detached Git `make check` with zero failures or ignored tests.

## R1c owned tools

- `DurableToolRegistry` seals offered native `Tool` definitions with implementation identity/version, replay policy and schema identity. It supports a bounded JSON Schema subset: object properties with string, number, integer or boolean values, required names and `additionalProperties: false`.
- The first model request carries the exact offered native schemas. A tool-call terminal persists the original assistant message, original provider arguments, separately validated execution arguments, stable durable idempotency key, parent `completing` checkpoint and owned child intents before any tool effect.
- Fresh unsafe tools execute once after their intent commit. Reopen replay requires stored and current `Safe` policy plus exact implementation ID, version and schema identity. Unsafe, changed or missing tools settle with a redacted durable interruption and no effect.
- Each child uses a durable `pending→running→terminal` phase. Tool result entry, child outcome and optional validated usage settle atomically from a fresh snapshot. Committed terminal children are reconstructed without re-execution.
- The successor model receives the original assistant tool call followed by committed native tool-result messages. Its complete transcript and attempt are committed on the still-`completing` parent before the provider effect. The final answer terminalises the parent/submission and updates live/inbox/usage atomically.
- Durable abort marks parent and children before signalling active tool cancellation, drains admitted non-cooperative work, then settles children before the parent. Close is separate from abort: it seals new phases, drains admitted tool settlement, starts no successor provider after sealing and releases storage only after owned work exits.
- Tool schemas, offered registry bytes, calls per round, rounds, arguments, outputs and outcomes have fixed preallocation limits. Invalid custom output/usage becomes a typed terminal failure rather than an application-data commit or implicit replay.

## Journal format

A fixed checksummed header is followed by digest-linked frames. Each frame stores magic/version, sequence, payload length, previous-frame digest, payload digest, canonical JSON batch and a trailer repeating sequence/length/digest. The writer validates and encodes the complete frame before append, uses `write_all`, calls `sync_all`, then acknowledges and adopts.

On reopen, a legal partial final-frame prefix is ignored and truncated after exclusive open. The scanner compares every available magic, sequence, chain and trailer byte before classifying a tail as partial. Arbitrary trailing junk and any fully present bad header/version/magic/checksum/trailer, sequence gap/rollback, hash-chain break, invalid reference or transition are corruption and fail closed. Existing empty files are corrupt rather than silently reinitialised. Newly created owned directory edges use mode `0700`, the journal uses `0600`, and each creation is parent-synchronised; existing host-owned parent permissions are never changed. The journal is single-process and plaintext. It claims no stale-PID detection, cross-process lease or encryption.

## Limits

- request ID/submission content: 1 MiB
- entry/message: 16 MiB
- document: 4 MiB
- each task input/checkpoint/outcome: 4 MiB
- atomic batch: 32 MiB

Sizes use encoded JSON bytes. There is no truncation.

## v1.1.0 tool model access

`ToolExecution.models` carries a shared `Arc<dyn DurableModels>` with model lookup, completion and classification methods. Existing harness constructors use `RegistryModels`; `open_with_tool_models` accepts a host service. Normal and recovered executions receive the same host object, which is process-local and never serialized into a tool intent. Tests verify object identity, a nested completion, recovery injection and default HTTP completion/Decisions dispatch.

The host owns credentials, cancellation/options and nested-call accounting. Nested usage is not automatically added to durable aggregate usage. Generic task model access, hooks, environment construction and progress-output windows are not implemented. `ToolExecution` gained a required `models` field for direct struct construction.

## v1.1.0 lifecycle clock

`DurableSession::open_with_clock` accepts a shared `LifecycleClock` (`Arc<dyn Fn() -> i64 + Send + Sync>`); `DurableHarness::open_with_tools_and_clock` forwards it. Existing constructors use wall-clock milliseconds. The clock stamps missing task starts/ends at commit admission, is called only when a timestamp is needed, and preserves existing starts through recovery. Invalid negative values fail storage validation without changing the snapshot. Execution durations continue to use monotonic `Instant`, independent of the host clock. `ModelRun.duration_ms` carries terminal stream timing through initial/successor answer settlement into `durationMs` entries; native context reads retain it after journal reopen. `ModelRun::one` defaults to no duration, while direct struct construction must include the new optional field.

## v1.1.0 snapshot scans

`StorageSnapshot::scan_entries`, `scan_tasks` and `scan_submissions` return owned pages filtered by conversation. Entries default to descending ID order; tasks and submissions default to ascending. `ScanCursor` stores the exclusive last ID and direction. Continuations can omit order, but a conflicting order fails. Legacy cursors without direction use the record type's default. Limits must be positive; page values are detached from snapshot JSON.

`EntryQuery` adds inclusive ID bounds and kind filtering; `TaskQuery` adds kind/state/abort filtering; `SubmissionQuery` adds status filtering. `DurableSession::entries/tasks/submissions` evaluate these on the mutation queue and clone only returned page records; the harness exposes root-conversation counterparts. Session `tasks_in`/`submissions_in` accept an optional conversation; None selects all native conversations, while existing task/submission methods and harness wrappers stay scoped. Filters, default/cursor direction and exclusive page boundaries apply to the combined ID order. Entry pagination seeks into the BTreeMap range before filtering. Reads wait for admitted commits, reject after close and fail when storage settlement has poisoned the session.

Fork traversal, conversation records, background-task filters, backend streaming and SQLite storage are absent. Rust cursor IDs use the existing positive-i64 contract, including zero as an exclusive boundary; upstream uses JavaScript safe integers. Regressions cover ordering, ranges, filters, pagination, isolation, detached values, read/commit ordering, poison/close and rejected-commit snapshot atomicity.

## v1.1.0 historical text context

`DurableHarness::context_with_options(ContextOptions { at: Some(entry_id) })` returns the native text context through a visible entry, inclusive. `at: None` matches `context()`. Unknown or foreign-conversation IDs fail; the query neither commits nor dispatches effects. Ordering follows the native `(created_seq, id)` sequence. Tests cut before a submission, at its user/answer entries and at the tail, then compare state before and after the query.

The compatibility `context()` helper includes user/assistant text only. `message_context(ContextOptions { at })` returns native messages, including persisted assistant tool calls and tool results. Calls persist atomically with pending children before effects. Results appear immediately after their assistant in call order; the first matching result before the next assistant wins, missing results are synthesized as errors, and duplicates/orphans are dropped. Result durations survive reconstruction. Legacy text-only assistant records remain readable.

New submissions persist native context in their intent so prior tool rounds survive follow-up dispatch and recovery. The native FIFO cutoff includes completed prior turns even when a queued input ID predates the prior answer, then places the active input last. Reconstructed messages do not acquire new timing. Current-message reads reuse appended ranges and keep raw open tool-result history; full ContextView/historical range reuse, fork ancestry and busy-boundary context updates are not implemented.

## Native context heads and edits

`update_context(ContextUpdate)` writes a native `context` entry while idle. Its optional head points to a visible prior entry or `ContextHead::SelfEntry(SelfHead::SelfEntry)`; omit/replace edits target earlier entries in the same conversation. Validation rejects future/foreign/non-entry targets, invalid edit shapes and oversized message/edit lists before mutation. A replacement contributes its message sequence at the target's position; latest visible edits win, including those recorded in older head markers. Only the newest visible head marker contributes messages, before the retained non-head entries; historical cuts apply only updates at/before their cutoff. Edits before the retained lower bound do not participate. After tool-result repair, a leading system contribution moves ahead of preceding user-only messages.

Both native provider context and the text-runner compatibility view apply these rules. Reads never dispatch. Reopen preserves updates, and cache invalidation follows successful commits. Updates during pending work reject. Idle harness head writes reject as stale if their numeric target precedes the current retained head; equal targets and self heads are allowed. Raw session batches construct history without this admission policy. Upstream queued boundary scheduling and forks are absent. The native journal kind/schema is an adaptation of upstream's flat EntryRecord wire format.

## Generic passive entries

`DurableSession::append_entry(conversation, EntryDraft)` allocates identity and sequence on the session mutation line. `DurableHarness::append_entry` exposes idle-only root-conversation writes. Session/harness `entry` reads return one detached record, or `None` for missing/foreign IDs, ordered with admitted commits and rejecting on poison/close. Drafts carry a non-empty kind (at most 64 UTF-8 bytes), optional JSON `data`, optional `model` messages, head and omit/replace edits. Native execution kinds are reserved; custom and upstream `pi.*` kinds are allowed. Missing data and explicit JSON null round-trip separately. `head: "self"` resolves to the assigned entry ID before persistence.

Generic records keep their custom kind and store draft fields inside native `EntryRecord.value`. They contribute context through the same head/edit/result-repair derivation, while model-less records remain visible with empty contributions. Storage validates JSON shape/size, model/edit counts, resolved heads and prior same-conversation edit references before mutation. Within an ordered CommitBatch, later entries can refer to earlier entries in that batch; forward, reordered and foreign references reject the whole batch. Journal replay applies the same rules. Rejected/dropped-before-admission writes consume no IDs; admitted writes settle and publish even after caller drop. Appends create no submission/task and journal reopen dispatches nothing.

`EntryDefinition<D>` is a process-local kind token: `draft(data)` builds a generic draft with required Serde-serializable data, `matches` tests only kind identity, and `decode` reads typed data or returns None for missing/foreign records. Matching malformed or absent data returns an error; nullable data uses `Option<D>`. Definitions require no registry and persist no schema. Use an untyped EntryDraft for model-only records without data.

Conversation existence/fork visibility, scoped task attribution, full task/document transactions and queued busy-boundary writes are unimplemented. The native session still accepts explicit conversation IDs without a conversation table; the harness restricts writes to its root.

## Atomic entry transactions

`DurableSession::transact_entries` runs a synchronous `FnOnce` on the session mutation line and returns its value after storage settlement. `EntryTransaction` borrows the admitted state, allows detached entry/task/submission table reads before writing, and stages generic appends with assigned IDs. The first append starts the write phase; later table reads reject with `table read after write`. Later appends may refer to earlier staged entries. Read-only callbacks consume no sequence and publish nothing.

Callback errors, caught staging failures, invalid final references and unwinding panics discard the whole batch without consuming IDs. Panics become rejected transactions; aborting-process panic handlers cannot be recovered. Staging enforces JSON shape/entry size and aggregate byte limits, followed by final storage validation. One adopted batch publishes after settlement. Cancellation before dequeue skips the callback; admitted settlement survives caller drop, close waits for it, and uncertainty poisons later operations.

Callbacks must be short, nonblocking and must not reenter their session. The borrowed handle cannot escape; external effects are the caller's responsibility and cannot be rolled back. Async callbacks, pending-operation drains, task/document/conversation writes and transaction task-attribution are absent. This is a native entry-only transaction API, separate from upstream's full Tx contract.

## Native context views

`DurableSession::context_view(conversation, at)` and `DurableHarness::context_view(ContextOptions { at })` return detached `ContextView` values on the session line. `head` contains the newest marker entry and resolved retained lower bound; `entries` lists that marker first, then retained non-head entries. `contributions` aligns one-for-one with those entries after edits and excluded assistant stop reasons, before tool-result repair or system-prefix promotion. Entries without model messages remain visible with an empty contribution; synthesized missing results appear only in the final `messages`.

Historical views are inclusive, reject foreign/missing cutoffs, and neither commit nor dispatch. Reads wait for admitted commits and reject on poison/close. The view does not reuse the messages-only cache or expose upstream generic EntryRecord/fork ancestry; callers pay for detached entries and contributions only when requesting a full view.

## v1.1.0 native context retention

`DurableSession::message_context` derives on the mutation queue. Current-context reads retain an immutable message prefix and raw open suffix, seeking only entries after the cached tail. Late tool results repair the open assistant suffix instead of preserving synthesized missing-result errors. New heads/edits rebuild conservatively; changed task input invalidates that conversation because legacy assistant model identity derives from it. Unrelated conversations and lifecycle/checkpoint-only commits do not invalidate a range. Historical cuts and full ContextView reads bypass this cache and never replace its current range. Results are cloned so callers cannot mutate retained messages.

Ranges are pinned while any native task in their conversation is nonterminal. The ten-minute expiry starts when adopted task state becomes idle; reads do not extend that deadline. Zero retention drops idle ranges immediately but permits bounded reuse while busy. Poison/close cleanup and the 64-conversation limit are preserved. A conservative saturating budget reserves three times the raw serialized messages plus per-message/tool-call overhead; each range has a 4 MiB budget and all ranges share 32 MiB. Budget overflow returns the context without retaining it. Rust object/allocator overhead is additional; these budgets are not measured live-heap limits. Only additions are measured, avoiding repeated serialization of the whole retained range. Raw and assembled messages require more peak heap than messages-only retention.

`DurableHarness::open_with_services` accepts `HarnessServices` containing the host tools, model service, lifecycle clock and session settings. Existing constructors keep their defaults. The range cache does not supply generation-task context, reuse historical/full-view ranges, or implement owned-conversation/fork visibility. Native busy detection includes pending/running/completing tasks but has no upstream background-task distinction. Those upstream contracts are open.

## Native committed-state watches

`DurableSession::watch` (and the harness wrapper) atomically acquires a detached snapshot and subscribes on the session queue. `DurableWatch::next` yields `Snapshot`, adopted `CommitBatch` and terminal `End`. No event appears before storage settlement/adoption; rejected commits publish nothing. Close and uncertain settlement end watches with `Closed` or `Poisoned`; `stop`/drop release slots without affecting durable work.

There are at most 64 active watches. Each queues at most 64 records or 32 MiB encoded commit deltas; overflow clears undelivered deltas and enqueues a fresh adopted snapshot. Snapshots can exceed that byte cap, as can their Rust object overhead. Receivers reconcile from the replacement snapshot and continue with later deltas. The native watch has no upstream `start(listener)` callback form, acquisition abort signal, transient progress or full AgentEvent run/message/tool/compaction taxonomy.

## Later required work

R1c supplies the executable tool registry, intent-before-effect, exact implementation/version/schema replay gate, generation→tool ownership, completing drain and bottom-up abort needed for the useful vertical. Same-version v1.0.1 retagging still requires local/hosted acceptance and publication authority; this document records implementation scope only.

Generic documents/tasks, forks, inbox modes, watches/events, extensions/hooks, durable partial output, deferred polling, compaction, subagents, remote storage, SQLite and cross-process leases belong to later cycles.
