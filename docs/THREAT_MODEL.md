# Relay: Threat Model

Last revised: 2026-08-30. Status of every control below: `planned`. Nothing
is built; no security claim is yet backed by passing evidence. Each control
names its enforcement point, evidence test family, and the gate (see
[BUILD_PLAN.md](./BUILD_PLAN.md)) at which its evidence becomes mandatory.
This document controls security-claim binding per [README.md](./README.md)
precedence; marketing may never state a property this document does not bind.

## 1. Scope and Security Objective

The threat model covers Relay, a self-hosted message queue and pub/sub
service: the `relayd` server binary, the RWP/1 wire API on port 7414,
metrics/health on port 7415, the Raft inter-node channel on port 7416, the
segmented WAL and snapshot files under the data directory, the `relayctl`
CLI, the `relay-client` library, and the build and release pipeline that
produces all of them.

Relay is deployed by an operator on infrastructure the operator controls. The
operator's network is semi-trusted: we assume the operator attempts to restrict
access, but we do not assume the network is free of hostile hosts, and every
listener treats its input as adversarial. Producers and consumers are
authenticated tenants of the same cluster, but tenants are mutually untrusted:
tenant A must not be able to read, delete, delay, forge, or starve tenant B's
messages, and must not be able to consume disproportionate cluster resources.

Primary objective: prevent an unauthenticated network peer, a malicious or
compromised authenticated tenant, a hostile Raft-port peer, or a compromised
dependency from reading or modifying message data, forging receipt handles,
corrupting replicated state, evading the audit log, or degrading service beyond
its enforced resource bounds.

Every security claim binds to a named enforcement point (a specific parser,
comparator, ACL check, or gate in a named crate) plus adversarial evidence (a
named test family in [OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md)); a
claim without both is not made, and every binding is `planned` today.

This is not a claim that Relay defends against a root-privileged operator on
its own host, against Byzantine cluster members (NG-07), or against kernel or
hardware compromise.

## 2. Assets

### 2.1 High-value assets

- Message bodies and attributes: customer data, present in memory, in WAL
  segments, in snapshots, and on the Raft channel
- Tenant credentials: per-tenant HMAC keys used for request authentication
  (FR-API-003)
- The receipt-handle signing key: the per-cluster HMAC-SHA256 key of
  [ADR-0006](./decisions/ADR-0006-ulid-ids-and-hmac-receipt-handles.md),
  whose compromise permits forgery against every queue
- TLS private keys for the API listener and the Raft channel
- WAL segment files (`wal-<seq>.seg`) and snapshot files (`snap-<lsn>.rsnap`):
  the durable record of every acknowledged message
- The administrative audit log (FR-ADMIN-008): the record of who changed what
- Cluster membership authority: the ability to add or remove Raft members
  (FR-REPL-006) is the ability to control quorum
- Build and release provenance: the binding between reviewed source, pinned
  dependencies, and the shipped `relayd` binary (FR-OPS-001, NFR-SEC-008)

### 2.2 Availability assets

- Broker CPU, memory, file descriptors, and connection table
- Disk capacity in the data directory; a full disk degrades every queue
- Raft quorum: loss of majority halts writes cluster-wide
- Per-tenant fair share of throughput and storage under quotas (FR-API-005)
- The operator's ability to observe, drain, and recover the cluster

## 3. Actors

- Authenticated producer: a tenant credentialed to send and publish
- Authenticated consumer: a tenant credentialed to receive, delete, and change
  visibility
- Network attacker on the API path: an unauthenticated peer able to reach port
  7414, observe, replay, or inject traffic
- Network attacker on the Raft path: a peer able to reach port 7416, attempt
  to join, impersonate, or partition the cluster
- Malicious or compromised tenant: holds valid credentials and uses every
  authenticated surface adversarially against other tenants or the broker
- Curious operator: reads what the filesystem permits; inside the trust
  boundary for data-at-rest confidentiality (Section 7.6), nothing else
- Compromised node in the cluster: a Raft member whose host is taken over;
  crash-stop faults are tolerated, arbitrary behavior is not (NG-07)
- Supply-chain attacker: compromises a dependency crate, a CI action, or the
  release channel
- Marketing-claims reviewer: an honesty control, not an adversary; audits
  every public claim against this document and [CORRECTNESS.md](./CORRECTNESS.md)
  under the MKT- family (FR-MKT-005)

## 4. Trust Assumptions

- The host operating system, filesystem, and disk hardware are trusted within
  documented limits; Relay detects accidental corruption (CRC) but does not
  defend its own host against root.
- Raft peers are trusted-but-authenticated: a node proving membership over
  mutual TLS is assumed crash-stop, not Byzantine (NG-07); in-protocol lying
  is outside the model.
- OS time may jump forward or backward; no state-machine decision depends on
  wall-clock reads (ADR-0005); wall time is used only for labels and logs.
- Operators can read the disk; at 1.0 there is no encryption at rest
  (Section 7.6, OQ-2 in [OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md)).
- The Rust standard library, cryptographic primitives, and the TLS
  implementation are trusted supply-chain code, exact-pinned (NFR-SEC-008).
- Tenants keep their own HMAC keys secret; a tenant that leaks its key has
  delegated its authority.

## 5. Trust Boundaries

```text
TB-1: Client (producer/consumer/relayctl user) <-> relayd API listener (7414, TLS 1.3, RWP/1)
TB-2: relayd node <-> relayd node, Raft channel (7416, mutually authenticated TLS)
TB-3: relayd process <-> data directory (WAL segments, snapshots, config)
TB-4: relayctl <-> administrative surface (admin opcodes over TB-1; read-only health/metrics on 7415)
TB-5: CI and release pipeline <-> published relayd/relayctl artifacts
```

Every boundary has versioned input, hard size limits, validation before
allocation, a stable rejection error (FR-API-006), and audited identity
wherever a mutation crosses it.

## 6. Data Flows

### 6.1 TB-1 — client to API listener

What crosses: RWP/1 frames carrying commands, each with the per-tenant HMAC
authenticator (FR-API-003); responses carrying message bodies, attributes,
and receipt handles. Parser at ingress: the `relay-wire` frame decoder. Bounded
parse: magic, then length checked against the 1 MiB frame cap, then CRC32C,
then opcode dispatch to a per-opcode fixed field layout; every variable-length
field is length-prefixed and checked against its documented limit before any
allocation (FR-API-002).

### 6.2 TB-2 — node to node, Raft channel

What crosses: Raft RPCs (vote, append, snapshot chunks) containing log entries,
which contain message bodies. Parser at ingress: the `relay-raft` message
decoder, same bounded-parse discipline as `relay-wire` — length-prefixed,
limit-checked before allocation, snapshot chunks capped at 1 MiB. Peers
complete mutual TLS authentication before any Raft message is parsed;
unauthenticated bytes never reach the decoder.

### 6.3 TB-3 — process to disk

What crosses: WAL records and snapshot chunks written by `relay-wal`; the same
read back at recovery. Parser at ingress (recovery is an ingress): the WAL
recovery reader validates segment header magic, format version, and per-record
`[len][crc32c]` framing; a record failing either check is treated as the torn
tail and truncated only at the tail (NFR-DUR-003). Snapshot reads validate
per-chunk CRC and the footer's
full-state SHA-256 before install. Disk content is trusted against malice
(Section 4) but never against corruption.

### 6.4 TB-4 — relayctl to administrative surface

What crosses: administrative commands (configuration, redrive, purge,
membership, leadership transfer) as RWP/1 admin opcodes, authenticated and
ACL-checked like tenant traffic, plus read-only health/metrics on 7415. Parser at ingress: the same `relay-wire` decoder; there is no second
admin parser to diverge from the fuzzed one. Every admin mutation writes an
audit record before its effect is acknowledged (FR-ADMIN-008).

### 6.5 TB-5 — CI to release artifacts

What crosses: source, the pinned lockfile, CI configuration, and the built
`relayd`/`relayctl` artifacts with embedded version and provenance
(FR-OPS-001). Validation at ingress: cargo-deny and lockfile review gate
dependency changes (NFR-SEC-008); artifacts carry checksums and provenance
verifiable against the tagged source.

## 7. Threats by Attack Surface

Entry format: attack, controls with enforcement point, evidence (named
planned test rows in [OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md)),
residual risk; the gate in parentheses is where evidence becomes CI-blocking.

### 7.1 Wire API parsing (TB-1)

**T-WIRE-01 — Malformed frame corrupts or crashes the decoder.**
- Attack: bytes with a valid magic but inconsistent length, wrong CRC, unknown
  opcode, or truncated body, seeking a panic, out-of-bounds read, or state
  confusion.
- Controls: `relay-wire` bounded parser (FR-API-002) validates magic, length
  cap, CRC32C, and opcode against the closed opcode table before touching the
  body; any violation closes the connection with one stable error code
  (FR-API-006) and no partial state change.
- Evidence: FUZZ-FRAME (coverage-guided fuzzing of the frame decoder with a
  checked-in corpus gating CI per NFR-SEC-002), WIRE-NEG (deterministic
  malformed-frame corpus with exact expected error codes) (R6).
- Residual risk: fuzzing is probabilistic; an unreached code path can hide a
  defect until the corpus grows.

**T-WIRE-02 — Oversize field causes unbounded allocation.**
- Attack: a length prefix claims a 4 GiB body, an attribute list claims 10,000
  entries, or a batch claims 1,000 messages, forcing allocation before
  validation.
- Controls: every length is checked against its documented limit (1 MiB
  frame, 256 KiB body, 10 attributes, batch 10, 80-char names) before any
  allocation, in `relay-wire` field readers; limits are constants shared with
  [PRODUCT_REQUIREMENTS.md](./PRODUCT_REQUIREMENTS.md), not re-derived.
- Evidence: FUZZ-LIMITS (mutation fuzzing of every length prefix), WIRE-LIM
  (deterministic boundary tests per limit: at, one under, one over) (R6).
- Residual risk: a parser-correct limit can still be mis-set against the
  memory budget; SOAK- runs bound aggregate memory (Section 7.5).

**T-WIRE-03 — Decompression bomb.**
- Attack: attacker sends a small frame that expands enormously when
  decompressed.
- Controls: structural absence — RWP/1 defines no compression in any frame or
  field ([ADR-0004](./decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md));
  wire length is memory length; the `relay-wire` opcode table has no
  decompression path to misconfigure.
- Evidence: WIRE-NEG rows asserting reserved flag bits are rejected, so a
  "compressed" flag cannot be smuggled in (R6).
- Residual risk: none short of a future protocol revision, which reopens this
  entry before shipping.

**T-WIRE-04 — Protocol-version confusion.**
- Attack: attacker negotiates an unknown or downgraded version to reach parsing
  behavior that was never fuzzed.
- Controls: version negotiation rejects unknown versions with a stable error
  before any state change (FR-API-009); exactly one wire version exists at 1.0,
  so there is no downgrade target.
- Evidence: WIRE-NEG version-mismatch rows; FUZZ-FRAME includes the negotiation
  exchange (R6).
- Residual risk: future multi-version support must re-run the fuzz corpus per
  version; a Section 11 review trigger.

### 7.2 Authentication and session (TB-1)

**T-AUTH-01 — Tenant key brute force.**
- Attack: iterate candidate HMAC keys against captured or live traffic.
- Controls: tenant keys are ≥ 256-bit random values generated by `relayctl`,
  never password-derived, making online and offline search infeasible; the
  authenticator covers the full frame; failed authentications are rate-limited
  per source and logged.
- Evidence: WIRE-AUTH rows for key-length enforcement at provisioning and for
  failed-auth throttling behavior (R6).
- Residual risk: a tenant that exports its key to a weak store defeats this
  outside Relay's boundary.

**T-AUTH-02 — Request replay.**
- Attack: attacker captures a validly authenticated frame and replays it later
  or on another connection.
- Controls: the HMAC authenticator binds the request ID and a client
  timestamp window enforced by the `relay-server` auth layer; frames outside
  the window, or repeating a request ID within it, are rejected before
  dispatch — see AB-3 for what replay rejection does and does not give.
- Evidence: WIRE-AUTH replay rows (same frame twice, same frame two
  connections, frame beyond window) (R6).
- Residual risk: within the window, a replay evading the request-ID cache on
  another node produces a duplicate, which at-least-once semantics already
  require consumers to tolerate (NG-01).

**T-AUTH-03 — MAC truncation.**
- Attack: attacker sends a frame whose authenticator field is shorter than 32
  bytes, hoping the server compares only the supplied prefix.
- Controls: the `relay-wire` layout fixes the authenticator at 32 bytes; any
  other length fails structural parsing before the comparator runs, and the
  comparator takes fixed-size arrays, not slices.
- Evidence: WIRE-AUTH truncation rows (0-, 1-, 16-, 31-byte tags all rejected
  as malformed, not as auth failures) (R6).
- Residual risk: none identified beyond implementation error, which the fixed
  test rows exist to catch.

**T-AUTH-04 — Timing side channel on credential comparison.**
- Attack: attacker measures response-time differences across authenticator
  guesses to recover the tag byte by byte.
- Controls: all credential and tag comparison uses a constant-time comparison
  primitive (NFR-SEC-004) in one audited function in the `relay-server` auth
  layer; no other code path compares secret bytes.
- Evidence: WIRE-AUTH-CT — a statistical timing test over correct-prefix
  versus wrong-prefix tags asserting no measurable correlation, plus a lint
  denying `==` on secret-typed byte arrays (R6).
- Residual risk: timing tests on shared CI hardware are noisy; the lint and
  code review carry the primary weight.

### 7.3 Authorization and ACL (TB-1, TB-4)

**T-ACL-01 — Confused deputy via topic fanout.**
- Attack: tenant A holds publish rights on topic T; a subscription binds T to
  tenant B's queue Q; A publishes to write into Q despite having no rights on
  Q, or A subscribes B's queue to a hostile topic to flood it.
- Controls: Subscribe (FR-TOPIC-002) requires subscribe rights on the topic
  and write-binding rights on the queue at subscribe time — the subscription
  records the queue owner's consent. Fanout then executes under the
  subscription's recorded authority, not the publisher's; deny precedence
  applies throughout (FR-API-004).
- Evidence: WIRE-ACL fanout rows: publish into a queue with no subscription
  authority fails at subscribe, never at publish; a revoked subscription stops
  delivery (R6 for ACL mechanics, R5 for fanout semantics).
- Residual risk: a queue owner who subscribes to a high-volume topic has
  consented to that volume; quotas (Section 7.5) bound the damage.

**T-ACL-02 — Tag-based authorization bypass.**
- Attack: if ACL rules can select resources by tag (FR-ADMIN-003), a tenant
  with tagging rights retags a resource into a rule that grants broader access.
- Controls: tag mutation is itself an ACL-guarded administrative operation;
  rules that match by tag are evaluated with deny precedence so a deny-by-name
  always beats an allow-by-tag; the ACL evaluator in `relay-server` is one
  function shared by every opcode, with a closed attribute set.
- Evidence: WIRE-ACL tag-mutation matrix: retagging by an unauthorized tenant
  fails; retagging by an authorized tenant never widens access past an explicit
  deny; ADMN- rows confirm the audit record for every tag change (R6).
- Residual risk: an over-broad allow-by-tag rule is an operator grant;
  `relayctl` warns on tag-based allows without a bounding deny.

**T-ACL-03 — Authorization checked against the wrong resource.**
- Attack: attacker names queue `a` in the authenticated envelope but targets
  queue `b` in the body, exploiting any place authorization and execution read
  different fields.
- Controls: the resource identifier is parsed once by `relay-wire` into the
  command object; the ACL check and the state-machine command consume that same
  object; there is no second parse between authorization and execution.
- Evidence: WIRE-ACL aliasing rows constructed from frames with conflicting
  duplicate fields, all rejected as malformed (R6).
- Residual risk: none identified beyond regression, which the rows pin.

### 7.4 Receipt handles (TB-1)

Receipt handles follow the construction fixed in
[ADR-0006](./decisions/ADR-0006-ulid-ids-and-hmac-receipt-handles.md):
`rh1_` + base64url( version u8 ‖ queue_id 16B ‖ message_id 16B ‖ lease_epoch
u64 ‖ expiry_nanos u64 ‖ HMAC-SHA256 tag 32B ), tag computed over all preceding
fields with the per-cluster receipt key. Property P-07 (RECEIPT-SAFE) and
NFR-SEC-001 own the guarantee.

**T-RCPT-01 — Handle forgery.**
- Attack: tenant constructs a handle for a message it never received, aiming to
  delete or re-time another tenant's message.
- Controls: the HMAC-SHA256 tag over version, queue_id, message_id,
  lease_epoch, and expiry_nanos; the `relay-server` handle validator verifies
  with the constant-time comparator (NFR-SEC-004) before trusting any field;
  the receipt key never leaves broker memory and rotates by key epoch
  (ADR-0006).
- Evidence: WIRE-RCPT forgery rows (random tags, tags from a wrong key,
  bit-flipped valid handles); FUZZ-RCPT fuzzes the handle decoder (R6).
- Residual risk: compromise of broker memory yields the receipt key; that is
  full-broker compromise, outside the tenant-vs-tenant model.

**T-RCPT-02 — Replay after delete.**
- Attack: consumer deletes with a valid handle, then replays the same handle to
  probe state or to race a redelivered copy.
- Controls: delete is idempotent for the same delivery (FR-QUEUE-006, P-06) —
  replaying the identical handle returns success without touching any other
  message; the handle cannot act on a later delivery because lease_epoch
  increments and the state machine validates epoch equality (FR-QUEUE-007).
- Evidence: MODL-RCPT rows checking delete-idempotence against the reference
  model; CORE- unit rows for epoch mismatch on every handle-consuming command
  (R1 for semantics, R6 for wire binding).
- Residual risk: none identified; idempotence is the specified behavior, not a
  leak.

**T-RCPT-03 — Cross-queue substitution.**
- Attack: tenant receives from queue X, then presents X's handle against queue
  Y where a colliding message_id might exist, or against a same-named recreated
  queue.
- Controls: queue_id (a 16-byte internal identifier, not the name) is inside
  the authenticated handle and must equal the target queue's identifier;
  DeleteQueue is terminal (FR-ADMIN-005) and a recreated queue has a fresh
  queue_id, so stale handles fail the queue binding, not merely lookup.
- Evidence: WIRE-RCPT substitution rows (handle from X against Y; handle from a
  deleted-and-recreated queue) (R6).
- Residual risk: none identified beyond receipt-key compromise, covered in
  T-RCPT-01.

**T-RCPT-04 — Lease-epoch reuse.**
- Attack: after a message returns to Available on visibility expiry and is
  redelivered, the previous consumer replays its old handle to delete a message
  now leased to someone else — a lease-exclusivity violation through the
  handle path (P-02).
- Controls: lease_epoch is a strictly increasing counter in the message's
  replicated state, incremented each delivery; every handle-consuming command
  rejects epoch inequality with the stable superseded-handle error
  (FR-QUEUE-007); because the epoch lives in the Raft-applied state machine,
  failover cannot resurrect an old epoch (P-09).
- Evidence: MODL-RCPT interleaving rows (deliver, expire, redeliver, old-handle
  delete rejected); SIM- schedules that force expiry/redelivery races,
  including across leader failover at R7 (R1, R6, R7).
- Residual risk: none identified within crash-stop assumptions; Byzantine
  state-machine divergence is out of scope (NG-07).

### 7.5 Resource exhaustion (TB-1)

**T-DOS-01 — Connection flood.**
- Attack: open connections until descriptors or buffers exhaust memory.
- Controls: hard connection limit and per-connection memory cap (NFR-SEC-006)
  in the `relay-server` accept loop; over-limit accepts are shed with the
  stable throttle error where possible, dropped otherwise (NFR-AVAIL-003).
- Evidence: WIRE-DOS connection-flood rows on an isolated runner; SOAK- runs
  assert flat memory under sustained connection churn (R6).
- Residual risk: shedding is visible degradation; the bound is no collapse,
  not no impact.

**T-DOS-02 — Slowloris.**
- Attack: attacker feeds one byte per interval, holding frames open forever.
- Controls: read/write deadlines per frame and bounded in-flight requests per
  connection (FR-API-010) in the connection driver; a connection that cannot
  complete a frame inside its deadline is closed.
- Evidence: WIRE-DOS slow-writer/slow-reader rows, deterministic via the
  injected `Clock` (ADR-0005) (R6).
- Residual risk: deadlines trade off against slow legitimate clients; values
  are configuration with documented floors.

**T-DOS-03 — Giant batches and payload maximization.**
- Attack: maximum-size batches of maximum-size bodies at maximum rate to
  crowd out other tenants.
- Controls: structural caps (batch 10, body 256 KiB, frame 1 MiB) bound the
  unit of work; per-tenant quotas and rate limits (FR-API-005) bound the rate;
  the per-queue in-flight cap (FR-QUEUE-016) bounds delivery-side state.
- Evidence: WIRE-DOS quota rows (tenant at quota gets the throttle error, a
  second tenant's latency stays in bound); BENCH- fairness runs (R6, R9).
- Residual risk: quota configuration is the operator's capacity statement;
  FR-OPS-011's capacity model is the honesty control, not a mechanism.

**T-DOS-04 — Dedup-window stuffing.**
- Attack: tenant floods a FIFO queue with unique deduplication IDs to grow the
  5-minute dedup index (FR-FIFO-007) without bound.
- Controls: entries expire at exactly 300 s, so the quota-admitted send rate
  bounds the live set to rate × window; the state machine accounts dedup-index
  memory against the queue's quota so the bound is enforced, not derived.
- Evidence: CORE- dedup boundary rows (P-05); SOAK- rows holding maximum
  admitted rate and asserting the dedup index plateaus (R4, R6).
- Residual risk: the plateau is proportional to admitted rate; an enormous
  quota purchases an enormous index.

**T-DOS-05 — FIFO group explosion.**
- Attack: tenant sends each message with a fresh MessageGroupId (≤ 128 bytes),
  growing per-group ordering state without bound.
- Controls: group state is reclaimed when a group has no Delayed, Available,
  or InFlight messages, so live group count is bounded by live message count,
  already bounded by the in-flight cap and quota; group-count metrics expose
  pressure under the cardinality budget (FR-OPS-004).
- Evidence: CORE- group-reclaim rows (empty group leaves no state); SOAK-
  one-message-per-group adversarial workloads asserting bounded memory
  (R4, R6).
- Residual risk: group churn degrades FIFO throughput, which is explicitly not
  promised to match standard throughput (NG-10).

**T-DOS-06 — Disk-fill by retention abuse.**
- Attack: tenant sends at quota with maximum retention (14 d) to fill the data
  directory, degrading every queue on the node.
- Controls: per-tenant storage quotas (FR-API-005) accounted against retained
  bytes, not just send rate; per-queue retention ceilings under operator
  control; disk-full fails writes cleanly while reads continue (NFR-DUR-004);
  the capacity model (FR-OPS-011) states the quota arithmetic that must hold.
- Evidence: CRSH- disk-full injection rows (R2); WIRE-DOS storage-quota rows
  (R6); OPSX- disk-watermark alerting rows (R8).
- Residual risk: quotas that sum past the disk are an operator arithmetic
  error; Relay surfaces the watermark but cannot prevent overcommitment.

**T-DOS-07 — Quota bypass through identity multiplication.**
- Attack: tenant spreads load across many connections or nodes to evade
  per-connection accounting.
- Controls: quotas key on the authenticated tenant identity, never the
  connection; replicated-operation accounting is applied in the state machine,
  so every node computes the same admitted set from the log.
- Evidence: WIRE-DOS multi-connection rows (N connections share one budget);
  RAFT- rows asserting quota decisions are log-consistent (R6, R7).
- Residual risk: many distinct tenant credentials multiply quota legitimately;
  credential issuance is the operator's control point.

### 7.6 Storage at rest (TB-3)

**T-STOR-01 — WAL or snapshot tampering below the process.**
- Attack: an actor with filesystem write access edits a WAL record or snapshot
  and fixes up the CRC32C, altering message content or acknowledged history.
- Controls, stated honestly: CRC32C on WAL records and snapshot chunks, and
  the snapshot footer's full-state SHA-256, are integrity-against-accident
  mechanisms — they detect torn writes, bit rot, and truncation (NFR-DUR-003).
  They are not cryptographic integrity: CRC32C is trivially forgeable and the
  SHA-256 footer is unauthenticated, so an actor who can write the file can
  produce a "valid" one. Relay 1.0 makes no at-rest tamper-proofing claim and
  no document may imply one ([MARKETING.md](./MARKETING.md) is bound here).
- Evidence: CRSH- corruption-detection rows prove the accident-detection claim
  (flip a byte, recovery truncates or refuses); no test claims more (R2).
- Residual risk: accepted in full — RR-2; the mitigation boundary is host
  security and file permissions, below.

**T-STOR-02 — Data directory readable or writable by other local users.**
- Attack: another local account reads WAL bodies or plants a modified
  snapshot.
- Controls: the data directory is created 0700 and verified at every startup,
  failing fast on violation (NFR-SEC-005), enforced in `relay-wal` recovery
  preflight; `relayd` documents running as a dedicated service user.
- Evidence: STOR-PERM rows (startup against 0755/0770/foreign-owner
  directories fails with the stable permission error; correct directory
  passes) (R2).
- Residual risk: root reads everything; 0700 bounds peers, not the
  administrator.

**T-STOR-03 — Sensitive data recoverable after deletion or uninstall.**
- Attack: an actor images the disk after messages were deleted or after Relay
  was removed; WAL history and compacted-away segments still hold bodies.
- Controls, stated honestly: delete removes a message from live state, not
  from history; WAL segments hold deleted bytes until compaction reclaims the
  segment (NFR-DUR-006); no secure-erase is performed; there is no encryption
  at rest at 1.0 (position and reopen trigger: OQ-2 in
  [OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md)); uninstall removes all Relay data
  (FR-OPS-009) but does not scrub free blocks.
- Evidence: documentation-honesty only — MKT- claims-audit rows verify no
  published text implies at-rest confidentiality or secure deletion (R9).
- Residual risk: accepted in full — RR-1; operators needing at-rest
  confidentiality must use full-disk encryption beneath Relay.

### 7.7 Raft channel (TB-2)

**T-RAFT-01 — Unauthenticated peer injection.**
- Attack: a network attacker on port 7416 sends AppendEntries or RequestVote
  to insert log entries, trigger elections, or read replicated message data.
- Controls: mutual TLS between nodes — each node authenticates the peer
  certificate against the cluster's node credentials before any Raft message
  is parsed; the `relay-raft` listener drops unauthenticated connections
  before the decoder. Membership (FR-REPL-006) is a replicated administrative
  decision, never implied by network reachability.
- Evidence: RAFT-SEC rows (plaintext peer, wrong-CA certificate, valid-CA
  non-member certificate, all rejected before decode; handler spy proves the
  decoder never ran) (R7).
- Residual risk: theft of a node's TLS key equals node compromise, covered by
  T-RAFT-04.

**T-RAFT-02 — Partition-forcing to break safety.**
- Attack: attacker who can delay or drop 7416 traffic engineers and heals
  partitions at adversarial moments, hunting double-lease or lost-ack windows.
- Controls: safety never depends on timing — leases are linearized through the
  replicated log (FR-REPL-004, P-08) and commit requires majority durable
  append (FR-REPL-002, P-09); a partition can suspend progress but no
  partition schedule can violate the properties.
- Evidence: SIM-RAFT partition schedules under the deterministic simulator,
  with seed-corpus regressions for every interesting schedule (NFR-MAINT-002);
  MODL- linearizability checks of histories recorded across partitions (R7).
- Residual risk: schedule exploration is probabilistic; the checked-in corpus
  ratchets coverage but cannot exhaust the space.

**T-RAFT-03 — Disruptive-server attacks.**
- Attack: a removed, lagging, or misconfigured node repeatedly triggers
  elections, destroying availability without violating safety.
- Controls: pre-vote always on (FR-REPL-001), so a node that cannot win a real
  election cannot force one; randomized election timeouts (500–1000 ms)
  prevent livelock; removed members are rejected at the TLS layer once the
  membership change commits.
- Evidence: RAFT- disruptive-server rows (partitioned node rejoins without
  deposing a healthy leader; removed node's connections refused) (R7).
- Residual risk: dropping traffic still denies availability; Relay claims
  availability under one node down (NFR-AVAIL-001), not under active attack.

**T-RAFT-04 — Compromised member node.**
- Attack: an attacker with full control of one member exfiltrates all
  replicated data and can lie in-protocol.
- Controls, stated honestly: none in-protocol — Relay is crash-stop, not
  Byzantine (NG-07); a compromised member reads every message and can corrupt
  state within what quorum accepts. Operational controls: single-server
  membership changes (FR-REPL-006) make eviction safe, membership actions are
  audited (FR-ADMIN-008), and the incident-response procedure (FR-OPS-012)
  covers eviction and receipt-key/TLS-credential rotation.
- Evidence: ADMN- eviction rows (evict, rotate, verify old credentials
  rejected) (R8, R10).
- Residual risk: accepted in full — RR-3; marketing must repeat NG-07 wherever
  cluster security is discussed.

### 7.8 Admin surface and relayctl (TB-4)

**T-ADMN-01 — Audit-log evasion.**
- Attack: an administrator performs a mutation that skips the audit record —
  racing a crash, a maintenance path, or an opcode that forgot to log.
- Controls: audit emission lives in the single admin-dispatch chokepoint in
  `relay-server`, not per-opcode, so an unaudited opcode cannot exist without
  bypassing the only dispatch path; the audit record is appended to the
  replicated log with the mutation, committing atomically with the effect and
  surviving failover; no maintenance path mutates state outside the log.
- Evidence: ADMN-AUDIT completeness rows (a generated test enumerates the
  opcode table, so a new opcode without an audit row fails CI); CRSH- rows
  proving no effect-without-record ordering exists (R8).
- Residual risk: an actor who edits the log on disk edits history — this is
  T-STOR-01/RR-2, not a separate audit guarantee.

**T-ADMN-02 — Leadership-transfer abuse.**
- Attack: an actor with cluster-admin rights transfers leadership repeatedly
  (FR-ADMIN-007) to degrade availability, or targets transfers to a node they
  control to position for T-RAFT-04.
- Controls: leadership transfer requires the cluster-admin ACL capability
  under deny precedence; transfers are rate-limited with the stable throttle
  error and audited with initiator identity (FR-ADMIN-008); transfer moves the
  leader role only, granting no data access membership did not already grant.
- Evidence: ADMN- transfer rows (unauthorized transfer denied; rapid transfers
  throttled; audit record present); RAFT- rows proving transfer preserves
  P-08/P-09 (R7, R8).
- Residual risk: a legitimate cluster admin is trusted with availability; the
  audit trail makes abuse attributable, not impossible.

**T-ADMN-03 — Purge and redrive as data-destruction weapons.**
- Attack: a hostile tenant purges a queue (FR-QUEUE-015) or redrives a DLQ
  (FR-QUEUE-019) to destroy or scramble another team's in-flight work.
- Controls: purge and redrive are distinct ACL capabilities, not implied by
  send or receive; concurrent purge is rejected while one is active
  (FR-QUEUE-015); both are audited with initiator identity.
- Evidence: WIRE-ACL capability-separation rows (send rights alone cannot
  purge); ADMN-AUDIT rows for both operations (R6, R8).
- Residual risk: an operator who grants purge broadly has made that grant; the
  capability separation exists so they do not have to.

### 7.9 Observability leakage (7415, logs, traces, bundles)

**T-OBS-01 — Message data in logs, metrics, or traces.**
- Attack: a log line, error string, metric label, or trace attribute carries
  a body, attribute value, key, or handle into weaker-access systems.
- Controls: message bodies, attribute values, credentials, and receipt handles
  are forbidden in every log, metric, trace, and error surface (NFR-SEC-003);
  enforcement is typed — body and secret types do not implement the display
  and serialization traits the logging layer accepts, so leaking requires a
  deliberate conversion that review can see; error taxonomy messages
  (FR-API-006) are static strings plus non-content identifiers.
- Evidence: OPSX-CANARY — canary tests run traffic with known sentinel bodies,
  keys, and handles, then scan every log line, metric exposition, trace
  export, and error response for raw, hex, and base64 forms of each sentinel
  (R6, re-run at R8 when tracing lands).
- Residual risk: canaries prove the tested encodings absent, not all possible
  transformations; the type-level ban is the primary control, canaries the
  regression net.

**T-OBS-02 — Diagnostic bundle exfiltration.**
- Attack: a support bundle from `relayctl diagnose` (FR-OPS-010), shared with
  a third party for help, carries message data or keys.
- Controls: the bundle is redacted at generation through the same layer as
  logging; the manifest lists every included file for pre-share review.
- Evidence: OPSX-CANARY bundle rows (sentinel-laden cluster produces a bundle;
  scan finds no sentinel in any encoding) (R8).
- Residual risk: WAL files are never bundled, but an operator who manually
  attaches them shares message data; the runbook warns explicitly.

**T-OBS-03 — Metrics cardinality abuse.**
- Attack: a tenant creates many distinct queue names, or a bug labels metrics
  by message ID, exploding cardinality until the pipeline or broker fails.
- Controls: a named cardinality budget governs every metric (FR-OPS-004,
  enumerated in [OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md)); labels
  come from a closed set — per-queue metrics exist only under a configurable
  top-N with the remainder aggregated; message-scoped labels are banned.
- Evidence: OPSX- cardinality rows (create 10,000 queues, assert exposition
  series count stays within budget); a lint over metric registration rejects
  unbounded label sources (R8).
- Residual risk: the top-N cutoff trades per-queue visibility for safety; the
  runbook documents how to widen it deliberately.

### 7.10 Supply chain and release (TB-5)

**T-SUPP-01 — Compromised dependency crate.**
- Attack: a crate in the dependency graph ships a malicious version, or a
  build script exfiltrates from CI.
- Controls: dependencies are exact-pinned in the lockfile, reviewed at a
  named gate, and carried with provenance (NFR-SEC-008); cargo-deny gates
  licenses, advisories, and duplicates in CI (ADR-0001); additions require a
  recorded decision; the dependency surface is kept small by the
  hand-rolled-core decisions
  ([ADR-0002](./decisions/ADR-0002-hand-rolled-segmented-wal.md),
  [ADR-0003](./decisions/ADR-0003-in-house-raft-implementation.md)).
- Evidence: CI cargo-deny and lockfile-drift checks (R0, then every gate);
  OPSX- release rows verifying the advisory scan ran for the released commit
  (R10).
- Residual risk: a compromised upstream release that predates its advisory
  passes the scan — RR-5; pinning bounds the window to deliberate upgrades.

**T-SUPP-02 — Non-reproducible or substituted build.**
- Attack: the released `relayd` differs from the reviewed source — CI
  compromise, poisoned cache, or substituted artifact.
- Controls: release builds run in CI from a tagged commit with pinned
  toolchain (MSRV 1.85) and pinned CI actions; the binary embeds version and
  build provenance (FR-OPS-001) and the release publishes checksums; the
  release checklist (BUILD_PLAN §17) verifies the chain from tag to artifact.
- Evidence: OPSX-REL rows: rebuild from the tag on a clean machine and compare
  the embedded provenance and, where the toolchain permits, the binary hash;
  a clean-machine install test validates published checksums (R10).
- Residual risk: full bit-for-bit reproducibility depends on toolchain
  determinism outside Relay's control — RR-6; provenance and checksums bound
  substitution even where bit-identity fails.

**T-SUPP-03 — Stolen publishing credential.**
- Attack: an attacker with the release credential publishes a malicious
  version to the release channel.
- Controls: publishing runs only from a protected CI environment with
  least-privilege short-lived credentials; no human holds a standing publish
  token; FR-OPS-012 defines how a bad release is revoked and announced.
- Evidence: OPSX-REL environment-audit rows (publish attempted outside the
  protected environment fails); a revocation drill executes the FR-OPS-012
  procedure once before 1.0 (R10).
- Residual risk: revocation cannot un-download a binary; the procedure bounds
  time-to-notification, not exposure to zero.

## 8. Abuse Cases

**AB-1 — Tenant A forges a receipt to delete tenant B's message.** A crafts
`rh1_` bytes naming B's queue_id and a plausible message_id. Expected outcome:
the handle validator's HMAC check fails in constant time and returns the
stable invalid-handle error; nothing about B's queue state is disclosed,
including whether the message_id exists (T-RCPT-01, WIRE-RCPT).

**AB-2 — Consumer extends a lease forever to starve the DLQ.** A hostile
consumer calls ChangeMessageVisibility in a loop so the message never expires,
never increments receive count, and never dead-letters. Expected outcome:
total visibility is capped at the 12 h maximum from first delivery —
extensions beyond the cap are rejected; on expiry the receive count increments
(FR-QUEUE-005) and maxReceiveCount moves the message to the DLQ
(FR-QUEUE-017). Detention is bounded by 12 h per delivery and 1,000
deliveries, never indefinite (MODL- lease rows, CORE- cap rows).

**AB-3 — Attacker replays captured Send frames.** Recorded authenticated
frames are replayed within and beyond the acceptance window. Expected outcome:
beyond the window, rejection at the auth layer; within it, request-ID cache
rejection. Stated honestly: Relay is at-least-once (NG-01) — a replay that
lands as a duplicate is indistinguishable from a producer retry, and consumers
must already be idempotent. Replay rejection protects quota and audit
accuracy; delivery correctness never rested on it (T-AUTH-02, WIRE-AUTH).

**AB-4 — Operator restores a stale backup and replays acknowledged history.**
A restore from an old snapshot + WAL archive (FR-OPS-007) resurrects messages
delivered and deleted after the backup point, and old receipt handles could
act on the resurrected state. Expected outcome: the restore procedure is
explicit — `relayctl` stamps a restore marker that advances the cluster's
receipt-key epoch (ADR-0006), so every pre-restore handle fails validation;
the restore is audited; readiness reports the restore epoch so the
discontinuity is visible. Data loss back to the backup point is real and is
reported, never papered over (OPSX- disaster-recovery drill rows, R10).

**AB-5 — Tenant stuffs the dedup window to suppress a victim's sends.** An
attacker who guesses B's deduplication IDs sends first with those IDs, so B's
real sends return the attacker's message ID as "duplicates" (FR-FIFO-007).
Expected outcome: dedup scope is per-queue and send requires write access to
that queue, so a tenant without write access cannot poison its window. Within
one shared queue, writers are mutually trusted by that grant; the docs state
that shared FIFO queues share a dedup namespace (WIRE-ACL rows).

**AB-6 — Tenant floods distinct FIFO groups to exhaust broker memory.** One
message per fresh MessageGroupId at maximum admitted rate. Expected outcome:
group state is reclaimed when a group empties, live group count is bounded by
the in-flight cap and quota, and memory plateaus (T-DOS-05, SOAK- rows).
Throughput degrades within NG-10's honesty bound; the broker stays up.

**AB-7 — A non-member node attempts to join the cluster.** A host on the
operator's network dials port 7416 with a self-signed certificate, then with a
stolen valid-CA certificate not in the membership set. Expected outcome: both
are rejected at the TLS layer before any Raft message is decoded; membership
changes only through the audited administrative path (T-RAFT-01, RAFT-SEC
rows).

**AB-8 — A support bundle is used to exfiltrate message data.** An insider
with `relayctl diagnose` rights, but no receive rights on a queue, generates
bundles hoping to read message bodies from logs or dumps. Expected outcome:
the bundle contains no bodies, attribute values, credentials, or handles in
any encoding; the canary scan is the proof (T-OBS-02, OPSX-CANARY rows).

## 9. Security Invariants and Tests

Mapping from security requirements to named planned evidence; the gate is
where evidence becomes CI-blocking and stays replayed thereafter
(NFR-MAINT-004).

| Requirement | Invariant | Primary planned evidence | Gate |
|---|---|---|---|
| NFR-SEC-001 | Receipt handles unforgeable and single-use per delivery (P-07) | WIRE-RCPT forgery/substitution rows, FUZZ-RCPT, MODL-RCPT epoch interleavings | R6 |
| NFR-SEC-002 | All wire input untrusted; bounded parse everywhere | FUZZ-FRAME, FUZZ-LIMITS, FUZZ-RCPT with checked-in corpus gating CI; WIRE-NEG deterministic corpus | R6 |
| NFR-SEC-003 | No secrets or message data in logs, traces, errors, diagnostics | OPSX-CANARY across logs, metrics, traces, errors, bundles | R6, re-run R8 |
| NFR-SEC-004 | Credential and tag comparison constant-time | WIRE-AUTH-CT statistical rows plus the secret-comparison lint | R6 |
| NFR-SEC-005 | Data directory 0700, verified at startup | STOR-PERM startup matrix | R2 |
| NFR-SEC-006 | DoS bounds: memory caps, deadlines, max frame, connection limits | WIRE-DOS matrix, SOAK- memory-plateau runs | R6 |
| NFR-SEC-007 | Threat model re-reviewed at every release gate | Gate checklists in BUILD_PLAN §X.9 cite the review; MKT- audit confirms | every gate, terminal R10 |
| NFR-SEC-008 | Dependencies exact-pinned, reviewed, carried with provenance | cargo-deny + lockfile-drift CI checks, OPSX-REL provenance rows | R0 onward, terminal R10 |

Fuzz corpus policy: every fuzz target (FUZZ-FRAME, FUZZ-LIMITS, FUZZ-RCPT)
has a checked-in corpus; CI replays it deterministically on every merge and
runs bounded new exploration nightly; any input that ever caused a failure is
minimized into the corpus before the fix merges, so no crash is fixed without
its regression input. The corpus only grows; removing an input requires the
same review as removing a test.

Redaction canary policy: OPSX-CANARY defines a fixed sentinel set — message
bodies, attribute values, tenant keys, receipt handles — in raw, hex, base64,
and base64url encodings. Every new observability surface joins the canary
scan in the same change; the scan enumerates surfaces from a registry, so an
unscanned surface fails CI, not review.

## 10. Residual Risk Register

Each entry: risk, why it is accepted, and the trigger that forces revisiting.
Marketing may not contradict any entry (FR-MKT-003 discipline applies).

- **RR-1 — No encryption at rest at 1.0.** Message bodies are readable with
  filesystem access; deleted data persists in WAL segments until compaction.
  Accepted: a bad key-management design is worse than an honest absence, and
  full-disk encryption beneath Relay covers the dominant need. Revisit: OQ-2
  trigger.
- **RR-2 — CRC is not tamper-proofing.** An actor with disk write access can
  alter history undetectably. Accepted: the disk is inside the trust boundary
  (Section 4), and authenticated storage would not defend against the same
  actor holding the key on the same host. Revisit: any off-host key design, or
  OQ-2 reopening.
- **RR-3 — No Byzantine tolerance (NG-07).** A compromised member reads all
  data and can corrupt state within quorum. Accepted: BFT multiplies protocol
  complexity against an adversary that already owns a production host.
  Revisit: never silently; only via a new machine-checked design and ADR.
- **RR-4 — Replayed Sends can duplicate within the acceptance window.**
  Accepted: at-least-once (NG-01) already requires idempotent consumers; the
  window bounds quota and audit distortion. Revisit: if OQ-5 reopens.
- **RR-5 — A compromised dependency version predating its advisory passes
  scanning.** Accepted: exact-pinning bounds exposure to deliberate upgrade
  moments and the dependency surface is small. Revisit: any advisory affecting
  a pinned version, per FR-OPS-012.
- **RR-6 — Bit-for-bit reproducible builds are not guaranteed.** Accepted:
  toolchain determinism is outside Relay's control; provenance and checksums
  bound substitution. Revisit: when the pinned toolchain documents
  reproducibility support.
- **RR-7 — Fuzzing and simulation are probabilistic.** An unexplored input or
  schedule can hide a defect despite green CI. Accepted: the corpus ratchet
  (Section 9, NFR-MAINT-002) makes every found failure permanent evidence.
  Revisit: every gate review re-examines corpus coverage.
- **RR-8 — Timing side-channel evidence is weak on shared CI.** The
  constant-time claim (NFR-SEC-004) rests on the audited comparator and lint,
  not the statistical test. Accepted: dedicated timing hardware is out of
  budget before 1.0. Revisit: R9 benchmark hardware could host a stronger
  harness.
- **RR-9 — Availability under active network attack is not claimed.** Dropped
  or delayed traffic denies service without violating safety. Accepted:
  NFR-AVAIL covers crash-stop faults, and safety (P-08, P-09) holds
  regardless. Revisit: any request to claim DoS resilience triggers the MKT-
  audit against this entry.

Each release reviews this register, links mitigations completed since the
last review, and confirms no published claim exceeds it.

## 11. Review Cadence

Per NFR-SEC-007, this threat model is re-reviewed at every release gate R0
through R10; the gate's acceptance-evidence checklist (BUILD_PLAN §X.9) cites
the review outcome, and a gate does not close without it. The R6 and R7
reviews are the deep ones: R6 activates TB-1 and TB-4 for real traffic, and R7
activates TB-2.

Out-of-cycle review is triggered by:

- Any new listener, opcode, or protocol version on any port
- Any change to receipt-handle construction, key handling, or key rotation
  (ADR-0006)
- Any new dependency in `relay-wire`, `relay-raft`, or the auth path
- Any security advisory affecting a pinned dependency (FR-OPS-012)
- Any addition of an observability surface, exporter, or bundle content
- Any discovered bypass of a control in this document, before the fix merges
- Reopening of any OPEN_QUESTIONS entry that names this document
  (OQ-1, OQ-2, OQ-8)

A change adding an asset, actor, boundary, or residual risk must land with
its new adversarial test rows in
[OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md); a threat-model edit
without evidence rows is an unearned claim (NFR-MAINT-005).
