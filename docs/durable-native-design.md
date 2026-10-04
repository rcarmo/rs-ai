# Native durable R1a foundation

R1a adds the storage/session kernel needed by the native pi-durable port. Model/tool execution and the user-visible durable feature belong to R1b/R1c.

## Implemented contract

- Atomic `CommitBatch` writes entries, complete tasks, submissions, request-ID mappings, documents and post-commit high-water counters.
- Non-zero IDs are restricted to `1..=i64::MAX`; allocation and payload limits fail before persistence.
- JSON values are serialised and detached at storage boundaries. Entry kinds are `user`, `assistant`, `tool_result` or `model_error`; task kinds are `generation` or `tool`; document kinds are `pi.live`, `pi.usage` or `pi.checkpoint`. Task and document versions start at 1.
- `MemoryStorage` is the reference backend. `JournalStorage` is the persistent cycle-1 backend and uses no new dependency.
- One writer claim exists per storage instance. Journal paths are canonicalised and exclusively owned in-process; a second alias/open fails. Close drops the file and releases path ownership even if the storage value remains retained.
- The session queue is bounded to 64 commands; the storage queue is bounded to 16.
- Caller drop before dequeue is a tombstone with no effect. After dequeue/admission, write, acknowledgement, adoption/poison and publication settlement continue under owned workers.
- `close()` seals public admission, rejects unadmitted work, waits for admitted settlement, closes storage and joins both workers. It writes no durable user abort or terminal task result.
- An uncertain storage result poisons reads and later dispatch until reopen.
- Submission states are `pending`, `done`, `failed` or `aborted`. `pending` has neither answer nor reason; `done` has an answer and no reason; `failed`/`aborted` have a reason and no answer. Only `pending` can transition, and terminal records are immutable.

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

R1b/R1c must add a public root conversation, idempotent submission handle, production model runner, executable tool registry, durable intent-before-effect, exact-version safe replay, generation→tool ownership/completing drain, abort and non-aborting close across effects, and crash/reopen answer proof. Those are mandatory before the user-required same-version v1.0.1 durable retag.

Generic documents/tasks, forks, inbox modes, watches/events, extensions/hooks, durable partial output, deferred polling, compaction, subagents, remote storage, SQLite and cross-process leases belong to later cycles. R1a provides neither a useful assistant nor full pi-durable parity.
