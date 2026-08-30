# ADR-0004: RWP/1 Custom Binary Protocol; No SQS Wire Compatibility

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: FR-API-001, FR-API-002, FR-API-006, FR-API-009, FR-API-010, NFR-SEC-002, NFR-SEC-006

## Context

Relay needs a wire protocol whose parser can be exhaustively bounded and
fuzz-gated in CI (FR-API-002, NFR-SEC-002): every length checked against a
limit before allocation, every failure mapped to one stable error code
(FR-API-006), and DoS bounds enforceable per frame (FR-API-010, NFR-SEC-006).
The obvious commercial shortcut — speaking the Amazon SQS wire API so existing
SDKs work unmodified — must be decided now, because it constrains everything
from the error taxonomy to the receipt-handle format (ADR-0006). This ADR
rejects it and records why.

## Decision

Relay defines RWP/1, a custom length-prefixed binary protocol served on port
7414 by `crates/relay-wire`, with this binding frame header:

```
[magic "RWP1" 4B][len u32 LE (max 1 MiB)][crc32c u32][opcode u16][flags u16][request_id u64][body]
```

Bodies are per-opcode fixed field layouts with length-prefixed variable
fields; no general-purpose serde framework touches the wire. Protocol version
negotiation happens before any state change, and unknown versions are rejected
with a stable error (FR-API-009). Relay's API is SQS-shaped in semantics where
the spine's requirements say so, but deliberately not SQS-wire-compatible.

## Alternatives Considered

- **SQS wire compatibility**: REJECTED. (1) Query-API baggage — the SQS wire
  is form-encoded query actions plus XML/JSON hybrid responses; implementing
  it faithfully imports a parsing surface (charset handling, duplicate
  parameters, XML entities) that cannot be bounded the way FR-API-002 demands.
  (2) Signature v4 scope — compatible SDKs require full SigV4: canonical
  request construction, credential scoping, date rolling, and chunked-payload
  signing, an authentication codebase larger than Relay's queue core and
  irrelevant to per-tenant HMAC (FR-API-003). (3) Semantics we refuse to fake —
  SDK-driven clients assume SQS behaviors Relay deliberately does not have
  (eventually-consistent approximate counts, SQS's exact throttling and error
  taxonomy, its dedup and visibility edge cases); Relay would have to
  replicate them bug-for-bug or silently diverge under a compatible wire, and
  a compatibility layer that lies about semantics is worse than none.
- **gRPC**: rejected because codegen plus the h2 stack (hyper, h2, prost,
  tonic) sits outside the fuzz budget — NFR-SEC-002 requires the CI-gating
  corpus to cover the wire parser, and Relay cannot own or exhaustively fuzz a
  transitive HTTP/2 implementation of that size; HPACK state and stream
  multiplexing also break the one-frame-one-bound model of FR-API-010.
- **JSON over HTTP as the primary protocol**: rejected on parser bounds —
  JSON permits unbounded nesting, unbounded numbers, and requires allocation
  before structural validation, contradicting the check-length-before-allocate
  rule; content-type and chunked-encoding handling widen the pre-auth surface.

## Consequences

- Easier: the codec in `crates/relay-wire` is small enough to fuzz to
  saturation (FUZZ- family gates CI); error taxonomy, quotas, and deadlines
  attach to a frame, not a request abstraction; CRC32C catches corruption
  before dispatch.
- Harder: no existing SDK works — `relay-client` must be written and every
  supported language later needs its own; adoption cost is real and priced in.
- Revisit when: OQ-1 (an HTTP/JSON gateway translating to RWP/1 at the edge,
  deferred in [OPEN_QUESTIONS.md](../OPEN_QUESTIONS.md)) is accepted — that
  adds a gateway without reopening RWP/1 as the primary wire; a superseding
  ADR is required to change the frame header after any release ships.
