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

`DurableSession::open_with_clock` accepts a shared `LifecycleClock` (`Arc<dyn Fn() -> i64 + Send + Sync>`); `DurableHarness::open_with_tools_and_clock` forwards it. Existing constructors use wall-clock milliseconds. The clock stamps missing task starts/ends at commit admission, is called only when a timestamp is needed, and preserves existing starts through recovery. Invalid negative values fail storage validation without changing the snapshot. Execution durations continue to use monotonic `Instant`, independent of the host clock.

## v1.1.0 snapshot scans

`StorageSnapshot::scan_entries`, `scan_tasks` and `scan_submissions` return owned pages filtered by conversation. Entries default to descending ID order; tasks and submissions default to ascending. `ScanCursor` stores the exclusive last ID and direction. Continuations can omit order, but a conflicting order fails. Legacy cursors without direction use the record type's default. Limits must be positive; page values are detached from snapshot JSON.

`EntryQuery` adds inclusive ID bounds and kind filtering; `TaskQuery` adds kind/state/abort filtering; `SubmissionQuery` adds status filtering. `DurableSession::entries/tasks/submissions` evaluate these on the mutation queue and clone only returned page records; the harness exposes root-conversation counterparts. Entry pagination seeks into the BTreeMap range before filtering. Reads wait for admitted commits, reject after close and fail when storage settlement has poisoned the session.

Fork traversal, conversation records, background-task filters, backend streaming and SQLite storage are absent. Rust cursor IDs use the existing positive-i64 contract, including zero as an exclusive boundary; upstream uses JavaScript safe integers. Regressions cover ordering, ranges, filters, pagination, isolation, detached values, read/commit ordering, poison/close and rejected-commit snapshot atomicity.

## v1.1.0 historical text context

`DurableHarness::context_with_options(ContextOptions { at: Some(entry_id) })` returns the native text context through a visible entry, inclusive. `at: None` matches `context()`. Unknown or foreign-conversation IDs fail; the query neither commits nor dispatches effects. Ordering follows the native `(created_seq, id)` sequence. Tests cut before a submission, at its user/answer entries and at the tail, then compare state before and after the query.

The compatibility `context()` helper includes user/assistant text only. `message_context(ContextOptions { at })` returns native messages, including persisted assistant tool calls and tool results. Calls persist atomically with pending children before effects. Results appear immediately after their assistant in call order; the first matching result before the next assistant wins, missing results are synthesized as errors, and duplicates/orphans are dropped. Result durations survive reconstruction. Legacy text-only assistant records remain readable.

New submissions persist native context in their intent so prior tool rounds survive follow-up dispatch and recovery. The native FIFO cutoff includes completed prior turns even when a queued input ID predates the prior answer, then places the active input last. Reconstructed messages do not acquire new timing. Fork ancestry, edits, head resets, full open-message assembly and incremental retention caching are not implemented.

## Later required work

R1c supplies the executable tool registry, intent-before-effect, exact implementation/version/schema replay gate, generation→tool ownership, completing drain and bottom-up abort needed for the useful vertical. Same-version v1.0.1 retagging still requires local/hosted acceptance and publication authority; this document records implementation scope only.

Generic documents/tasks, forks, inbox modes, watches/events, extensions/hooks, durable partial output, deferred polling, compaction, subagents, remote storage, SQLite and cross-process leases belong to later cycles.
