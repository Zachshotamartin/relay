# Relay: Open Questions

Last revised: 2026-08-30. A forward-looking register of decisions the plan
intentionally defers. Each entry names its question, its current fail-closed
default position, and the concrete trigger that reopens it. Closing an entry
requires a new ADR under `docs/decisions/` and, where the entry touches a
trust boundary, new [THREAT_MODEL.md](./THREAT_MODEL.md) sections and test
rows before implementation. An entry may not be silently implemented ahead of
its trigger; doing so is a documentation-discipline violation (NFR-MAINT-005).

### OQ-1 — HTTP/JSON gateway

**Question.** Should Relay expose an HTTP/JSON API alongside RWP/1 for clients
that cannot speak a binary protocol?
**Fail-closed default position.** None. RWP/1 is the only client protocol
([ADR-0004](./decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md)); the
fuzz budget (NFR-SEC-002) covers exactly one parser, and a second ingress
would need its own bounded-parse and fuzz story before it exists.
**Reopen trigger.** Demonstrated user demand after 1.0, with a design that
gives the gateway its own THREAT_MODEL §7 surface and FUZZ- family.

### OQ-2 — Encryption at rest

**Question.** Should WAL segments and snapshots be encrypted on disk?
**Fail-closed default position.** None at 1.0, documented plainly: file
permissions (0700, NFR-SEC-005) and the operator's full-disk encryption are
the only at-rest controls, per THREAT_MODEL §7.6 and RR-1/RR-2. No document
may imply at-rest confidentiality (FR-MKT audit enforces this).
**Reopen trigger.** An enterprise requirement, or an accepted key-management
design covering key storage, rotation, restore, and key-loss behavior.

### OQ-3 — io_uring storage backend

**Question.** Should `relay-wal` use io_uring on Linux instead of the portable
pwrite/fsync path?
**Fail-closed default position.** Portable pwrite/fsync path only; io_uring is
optional per [ADR-0011](./decisions/ADR-0011-supported-platforms.md) and the
portable fallback is mandatory regardless. One IO path keeps CRSH- injection
coverage honest.
**Reopen trigger.** An NFR-PERF miss (NFR-PERF-001/002/004) on the reference
hardware attributable to the IO path, with CRSH- coverage extended to the new
backend before it ships.

### OQ-4 — Multi-Raft / per-queue groups for horizontal scale

**Question.** Should queues shard across multiple Raft groups so throughput
scales beyond one log?
**Fail-closed default position.** A single Raft group per cluster at 1.0.
Every safety property (P-08, P-09) is proven against one log; sharding
reintroduces cross-group ordering and membership questions the simulator does
not yet model.
**Reopen trigger.** A sustained requirement above 20,000 msg/s (the
NFR-PERF-001 envelope) from a real deployment, plus SIM- coverage extended to
multi-group schedules before any implementation.

### OQ-5 — Exactly-once / transactional producer API

**Question.** Should Relay offer exactly-once delivery or producer
transactions?
**Fail-closed default position.** Refuse. NG-01 stands: Relay is
at-least-once and consumers must be idempotent; every marketing surface
repeats this (FR-MKT-003). Pretending otherwise is the exact dishonesty the
project exists to avoid.
**Reopen trigger.** Never without a new machine-checked design — a reference
model, SIM- and MODL- coverage, and an ADR — accepted before any API is
sketched. Demand alone does not reopen this entry.

### OQ-6 — Windows support

**Question.** Should relayd/relayctl support Windows?
**Fail-closed default position.** Unsupported at 1.0 (ADR-0011). fsync
semantics, file locking, and path behavior differ enough that untested
support would silently weaken the durability contract (ADR-0008).
**Reopen trigger.** Demonstrated demand plus CI capacity for a full CRSH- and
STOR- matrix on Windows; tier promotion requires the same evidence Linux has.

### OQ-7 — Message priority levels

**Question.** Should messages carry a priority that reorders delivery?
**Fail-closed default position.** None. FIFO groups and per-message delay
(FR-QUEUE-010) are the only ordering controls. Priority interacts with
P-03 (eventual delivery) — starvation of low-priority messages would need a
liveness story the simulator would have to check.
**Reopen trigger.** Post-1.0 demand, with a design proving P-03 holds under
priority via SIM- liveness schedules.

### OQ-8 — Tiered / object-storage offload for retention

**Question.** Should old messages offload to object storage to extend
retention past local disk?
**Fail-closed default position.** Local disk only; retention is bounded at
14 d (FR-QUEUE-014) and the capacity model (FR-OPS-011) governs what fits.
Object storage adds a network dependency inside the durability contract,
which ADR-0002 deliberately excludes; it would also add a new trust boundary
requiring THREAT_MODEL coverage.
**Reopen trigger.** Real demand for retention beyond 14 d, with a design that
keeps the fsync-before-ack contract (ADR-0008) entirely local.

### OQ-9 — Dedup window configurability

**Question.** Should the FIFO deduplication window be configurable instead of
fixed at 300 s?
**Fail-closed default position.** Fixed 300 s. P-05 (DEDUP-EXACT) is proven
at exactly this window's boundaries; a variable window multiplies the
boundary cases and the dedup-index bound (THREAT_MODEL T-DOS-04) is sized
against the fixed value.
**Reopen trigger.** Evidence that the CORE- boundary tests and the SOAK-
index-plateau bound hold parameterically across the proposed range, before
the configuration knob exists.

### OQ-10 — Marketing website scope

**Question.** Should Relay have a marketing website beyond the repository?
**Fail-closed default position.** README and docs only until the R9 claims
audit exists. [MARKETING.md](./MARKETING.md) governs copy, and no launch
collateral ships before the MKT- claims-audit checklist can gate it
(FR-MKT-004, FR-MKT-005); a website before the audit is a claims surface
without its honesty control.
**Reopen trigger.** R9 acceptance, which brings the claims audit, published
benchmarks, and the failure-injection report the site would cite.

Review this register at every gate boundary alongside the threat-model review
(NFR-SEC-007). Closing an entry follows the reversal rule: a new ADR, never
an edit to an accepted one.
