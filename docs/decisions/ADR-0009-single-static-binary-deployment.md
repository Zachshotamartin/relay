# ADR-0009: Single Static Binary Deployment

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: FR-OPS-001, FR-OPS-002, FR-OPS-008, FR-OPS-009, NFR-AVAIL-004

## Context

Relay is self-hosted software operated by people who did not write it. Every
process, sidecar, and external dependency in the deployment multiplies the
operator's failure modes, the threat model's surface, and the test plan's
matrix. The R10 release gate requires installation, upgrade, rollback, and
uninstall procedures that are actually rehearsed (FR-OPS-008, FR-OPS-009);
those rehearsals are tractable only if the deployable unit is small and
closed. The decision also interlocks with ADR-0003 (no external coordination
service) and must precede BUILD_PLAN packaging work.

## Decision

Relay ships as one release artifact containing two statically linked
executables built from the same workspace at the same version: `relayd` (the
server, embedding version and build provenance per FR-OPS-001) and `relayctl`
(the admin CLI, a client of `relayd`'s API). A `relayd` process serves the
client API (port 7414), metrics and health (7415), and Raft inter-node
traffic (7416) from one binary; clustering is the same binary run three
times. Configuration is TOML file + environment + flags with fixed precedence
and fail-fast validation (FR-OPS-002). There are no required sidecars, no
external database, no coordination service, and no dynamic-library
dependencies beyond the platform baseline defined in ADR-0011.

## Alternatives Considered

- **Modular services** (separate broker, metadata/consensus, and gateway
  processes): rejected. Each split adds an internal network boundary that
  needs its own authentication, timeout, and version-skew story — pure
  operator-facing surface with no corresponding guarantee; deterministic
  simulation would have to model inter-service transport that exists only
  because of the packaging choice; and the rolling-upgrade matrix (FR-OPS-008)
  goes from one binary version pair to a cross-product of service versions.
- **Server plus mandatory external metadata store**: rejected in ADR-0003 for
  consensus and re-rejected here for packaging — it makes Relay's install,
  backup, and upgrade procedures hostage to a second system's.
- **Container-image-only distribution**: rejected as the sole channel — an
  image is published, but self-hosted operators on bare VMs get the same
  guarantees from a tarball with two binaries; requiring a container runtime
  would be an unforced dependency.
- **Separate versioned releases for `relayd` and `relayctl`**: rejected —
  independent versioning creates a compatibility matrix between the CLI and
  server for zero benefit; one artifact, one version, and `relayctl` speaks
  the same negotiated protocol as any client (FR-API-009).

## Consequences

- Easier: install is copy-two-binaries; rollback is re-copy-the-old-ones plus
  the WAL downgrade policy of NFR-DUR-007; the threat model has one process
  boundary and three well-known ports; the disaster-recovery drill
  (FR-OPS-007) scripts against a single unit.
- Harder: everything shares one process — a defect anywhere can take down all
  roles at once, so graceful shutdown and drain (NFR-AVAIL-004) and the
  crash-on-fsync-failure rule (ADR-0008) must be safe in one address space;
  binary size grows with every embedded feature.
- Revisit when: a future multi-tenant control plane genuinely needs an
  independent lifecycle — that is a superseding ADR with its own security
  review, not a drift. No OPEN_QUESTIONS entry reopens this decision.
