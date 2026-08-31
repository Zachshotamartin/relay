#![forbid(unsafe_code)]
//! Segmented write-ahead-log shell. Storage behavior is owned by R2; all bytes
//! will pass through the `relay-core` disk boundary rather than direct file IO.
//! Async runtimes, host clocks, networking, randomness, Raft, and server code are
//! forbidden dependencies.
