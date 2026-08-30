# ADR-0006: ULID Message IDs and HMAC-SHA256 Receipt Handles

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: FR-QUEUE-004, FR-QUEUE-006, FR-QUEUE-007, NFR-SEC-001, NFR-SEC-004, P-06, P-07

## Context

Two identifier formats must be fixed before R1, because the model checker's
history format and the receipt-validation rules both depend on them. Message
IDs must be sortable for operational locality and generated deterministically
in simulation. Receipt handles carry more weight: they are the capability that
authorizes delete and change-visibility, they must be unforgeable and
single-use per delivery (FR-QUEUE-007, NFR-SEC-001, P-07), and a stale handle
from a previous delivery of the same message must fail cleanly rather than
delete a message another consumer now holds. Storing server-side handle state
per delivery is possible but adds replicated state; a stateless authenticated
handle avoids that if its construction is exactly right, so the construction
is recorded here byte-for-byte.

## Decision

Message IDs are ULIDs (128-bit, Crockford base32) whose time component comes
from the log-applied clock (ADR-0005) and whose randomness comes from the
injected `Rng`, so IDs are deterministic in simulation and sortable in
production.

Receipt handles use this binding construction:

```
rh1_ + base64url( version u8
                ‖ queue_id 16B
                ‖ message_id 16B
                ‖ lease_epoch u64
                ‖ expiry_nanos u64
                ‖ HMAC-SHA256 tag 32B )
```

The HMAC key is per-cluster, stored in replicated cluster state, and rotated
via key epoch: rotation installs a new key epoch, and the previous epoch's key
remains valid for verification for the maximum visibility timeout (12 h) so
outstanding handles verify, after which it is destroyed. Single-use is
enforced by `lease_epoch`: it increments on each delivery of a message, and
delete/change-visibility validate epoch equality against current state, so a
handle from any earlier delivery is rejected (FR-QUEUE-007). Handle
verification compares HMAC tags in constant time (NFR-SEC-004). Delete with
the current valid handle is idempotent (P-06).

## Alternatives Considered

- **UUIDv7 message IDs**: rejected narrowly. Equivalent sortability, but ULID's
  Crockford base32 rendering is shorter and case-insensitive for operators,
  and the spine's history format already fixes ULID rendering in JSONL
  fixtures; carrying two canonical renderings buys nothing.
- **Random opaque tokens with server-side handle state**: rejected because
  every delivery would write handle state into the replicated log and every
  validation would read it, adding log volume and a replicated lookup to the
  hot path, when HMAC gives the same unforgeability statelessly; the epoch
  check against lease state is already required either way.
- **Unauthenticated handles** (encode message ID and lease data without a
  tag): rejected outright — any client could forge a handle for any message
  it can name and delete messages it never received, violating NFR-SEC-001
  and P-07.
- **Encrypting the handle instead of authenticating it**: rejected; secrecy of
  the fields is not a goal (queue and message IDs are visible to the caller
  anyway), integrity is, and authenticated encryption would add nonce
  management for no additional guarantee.

## Consequences

- Easier: handle validation is pure computation plus one lease-epoch
  comparison, testable in CORE-/MODL- families without IO; security tests can
  bit-flip every field and assert rejection (WIRE-, security families).
- Harder: the receipt key becomes replicated secret material — backup,
  restore, and the `relayctl diagnose` redaction rules (NFR-SEC-003) must all
  treat it as such; key-epoch rotation is an operator procedure that must be
  documented and drilled at R8.
- Revisit when: a field must be added to the handle — that is a `rh2_` prefix
  and a version bump, with `rh1_` verification retained for one release. No
  OPEN_QUESTIONS entry reopens this decision.
