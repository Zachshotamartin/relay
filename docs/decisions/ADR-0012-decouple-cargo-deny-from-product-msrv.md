# ADR-0012: Decouple cargo-deny from the Product MSRV

- Status: proposed
- Date: 2026-08-30
- Related findings or requirements: NFR-SEC-008, NFR-MAINT-004

## Context

The operations plan pins `cargo-deny` 0.16.4 and installs it from source with
Rust 1.85.0. That binary can no longer read the live RustSec advisory database:
it rejects the CVSS 4.0 vector in `RUSTSEC-2026-0073` with `unsupported CVSS
version: 4.0`. Keeping the old binary would therefore turn the required
advisory check red even when Relay's dependency graph is valid.

The first upstream release that fixes this parser incompatibility is
`cargo-deny` 0.18.6, whose MSRV is Rust 1.88.0. The selected `cargo-deny`
0.20.2 release also requires Rust 1.88.0 to compile. Raising Relay's product
MSRV solely for a host-side verifier would reverse ADR-0001, while pinning an
old advisory-database revision would suppress current security evidence. The
audit must fail closed without doing either.

## Decision

If accepted, this ADR supersedes only the `cargo-deny` 0.16.4 installation pin
in `OPERATIONS_TEST_PLAN.md`; Relay's workspace and product MSRV remain Rust
1.85.0. CI and contributor bootstrap will use `cargo-deny` 0.20.2 as a host
audit tool installed from the upstream release archive for the current host.

Every archive is selected only for an ADR-0011-supported host, downloaded from
the immutable `0.20.2` GitHub release URL, and verified before execution against
these exact SHA-256 digests:

| Host archive | SHA-256 |
| --- | --- |
| Linux x86_64 musl | `9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f` |
| Linux aarch64 musl | `995c82be0defc7a025cae49a2aa2644ce8245c9a3318fc4103907c6a285e8c7d` |
| macOS aarch64 | `fe67d82a10d8597a3549364cb733a3f9cc1bfff9031b7ae46384a9f2a72090c3` |

A hash mismatch, unavailable archive, unsupported host, or version mismatch
fails closed. No mutable tag, latest-release lookup, or stale advisory database
is an allowed fallback. The binary is not linked into Relay, does not enter
Relay's `Cargo.lock`, is not shipped in a product artifact, and invokes the
pinned Cargo 1.85 toolchain when inspecting Relay's locked dependency graph.

The `deny.toml` policy retains `unmaintained = "workspace"`, which 0.20.2
supports, and the live RustSec database remains authoritative. This is a narrow
host-verifier exception to the source-install rule, not permission to introduce
prebuilt product dependencies or lifecycle downloads.

## Alternatives Considered

- **Keep 0.16.4 or use the Rust-1.85-compatible 0.18.3 release**: rejected;
  both use a RustSec parser that fails on current CVSS 4.0 advisories.
- **Pin an older RustSec advisory database**: rejected because a green audit
  would omit current vulnerability data and cease to prove NFR-SEC-008.
- **Raise Relay's MSRV to Rust 1.88.0**: rejected because a host verifier does
  not justify changing the compiler contract for operators and contributors.
- **Maintain a patched cargo-deny fork**: rejected because it creates an
  unaudited security-tool maintenance burden and a second update channel.

## Consequences

- Easier: the audit follows the live RustSec database while every Relay crate
  remains buildable with the ADR-0001 toolchain.
- Harder: verifier upgrades require a dedicated supply-chain review that
  records the exact version, release provenance, supported-host archives,
  digests, and a cold advisory check.
- Revisit when: an upstream cargo-deny release that parses the live database
  can again be source-built with Relay's product MSRV, or Relay's product MSRV
  is independently raised by a superseding ADR.
