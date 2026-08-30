# ADR-0010: Observability — Prometheus Metrics, OTLP Traces, JSON Logs

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: FR-OPS-003, FR-OPS-004, FR-OPS-005, FR-OPS-006, FR-OPS-010, NFR-SEC-003

## Context

Relay's operability gate (R8) requires that an operator can answer "is it
healthy, where is the latency, what happened at 03:14" without reading source
code. Self-hosted deployments mean Relay cannot assume any particular vendor
backend; it must emit signals in formats the operator's existing stack already
ingests. The formats are fixed now because instrumentation retrofitted after
R6's server work is invariably incomplete, and because the redaction rules
(NFR-SEC-003) must apply to every signal from the first line of server code.

## Decision

Relay emits exactly three signal types from `relayd`, all on infrastructure
the operator already runs:

1. **Prometheus metrics**, exposed in text exposition format on port 7415
   alongside health and readiness (FR-OPS-003), under a named cardinality
   budget defined in [OPERATIONS_TEST_PLAN.md](../OPERATIONS_TEST_PLAN.md) —
   label sets are enumerated per metric, per-message and per-tenant-unbounded
   labels are forbidden, and an OPSX- test fails CI when a metric exceeds its
   budget.
2. **OTLP trace spans** across the request lifecycle (FR-OPS-005) — frame
   decode, auth, core apply, WAL sync, ack — exported over OTLP/gRPC to a
   collector endpoint the operator configures; tracing is off unless an
   endpoint is set.
3. **Structured JSON logs** on stderr with stable field conventions
   (FR-OPS-006): fixed keys for timestamp, level, target, request ID, queue,
   and error code, so log pipelines parse without per-release regex churn.

All three pass through the same redaction layer: secrets, credentials, and
message bodies never appear in any signal, verified by canary tests
(NFR-SEC-003), and `relayctl diagnose` bundles only redacted output
(FR-OPS-010).

## Alternatives Considered

- **Logs only, no metrics or traces**: rejected — answering "what is p99
  send-to-ack right now" from logs requires the operator to build aggregation
  Relay should have shipped; NFR-PERF verification at R9 needs first-class
  histograms anyway.
- **StatsD/DogStatsD emission**: rejected — push-based UDP loses data
  silently under exactly the overload conditions being diagnosed, and the
  protocol's lack of native labels forces metric-name explosion that defeats
  the cardinality budget.
- **Vendor APM agent**: rejected — a closed-pipeline dependency in a
  self-hosted product ties operators to a vendor and inserts unauditable code
  into a process whose determinism and redaction guarantees are load-bearing;
  OTLP reaches every major vendor anyway.
- **OpenTelemetry for metrics too** (drop the Prometheus endpoint): rejected
  for 1.0 — a scrapeable endpoint works with zero collector infrastructure,
  which matches the single-binary operator story (ADR-0009); OTLP metrics can
  be added later without removing the endpoint.
- **Wide-events-only logging** (one fat event per request, no metrics):
  rejected — elegant for debugging but leaves alerting and SLO evaluation to
  a query engine the operator may not have; it also multiplies log volume at
  the 20,000 msg/s target.

## Consequences

- Easier: any Prometheus + Grafana + OTLP collector stack works on day one;
  the runbook (R8) can reference exact metric names; the cardinality budget
  makes memory use of the metrics registry predictable and testable.
- Harder: three emission paths must each honor redaction and be canary-tested;
  span overhead on the hot path must be measured at R9 so tracing cost is a
  published number, not folklore.
- Revisit when: OTLP metrics maturity makes a single-protocol pipeline
  compelling — additive change, ordinary ADR; removing the Prometheus
  endpoint would be breaking and requires a superseding ADR. No
  OPEN_QUESTIONS entry reopens this decision.
