#![forbid(unsafe_code)]
//! Independent reference-model and linearizability-oracle shell. Model behavior
//! is owned by R1. WAL, Raft, server, and async-runtime dependencies are
//! forbidden so the oracle cannot import the implementation it judges.
