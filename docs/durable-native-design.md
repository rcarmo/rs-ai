# Native durable R1a/R1b foundation

R1a adds the storage/session kernel. R1b adds persistent no-tool model generation through the existing rs-ai provider registry. Executable tools and model→tool→answer ownership belong to R1c.

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

## Later required work

R1c must add an executable tool registry, durable intent-before-effect, exact implementation/version safe replay, generation→tool ownership, `completing` drain and bottom-up abort. Those remain mandatory before a same-version v1.0.1 durable retag. R1b alone is a persistent no-tool generation slice, not the useful-port completion.

Generic documents/tasks, forks, inbox modes, watches/events, extensions/hooks, durable partial output, deferred polling, compaction, subagents, remote storage, SQLite and cross-process leases belong to later cycles.
