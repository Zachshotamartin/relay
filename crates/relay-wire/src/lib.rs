#![forbid(unsafe_code)]
//! Bounded RWP/1 codec shell. Protocol parsing, authentication, and fuzzing are
//! owned by R6. IO, async runtimes, general-purpose wire serialization, and any
//! allocation made before attacker-controlled lengths are validated are
//! forbidden.
