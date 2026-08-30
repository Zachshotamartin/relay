#![forbid(unsafe_code)]
//! In-house Raft protocol shell. Consensus behavior is owned by R7; wire and
//! server dependencies are forbidden from its pure protocol core.

