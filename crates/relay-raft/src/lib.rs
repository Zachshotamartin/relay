#![forbid(unsafe_code)]
//! In-house Raft protocol shell. Consensus behavior is owned by R7. Async
//! runtimes, host clocks, ambient randomness, wire code, and server code are
//! forbidden from its pure protocol core.
