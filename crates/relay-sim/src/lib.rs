#![forbid(unsafe_code)]
//! Deterministic simulation shell. Virtual execution and replay are owned by R3.
//! Production async schedulers, real sockets, real files, and wall-clock reads
//! outside the top-level wall-budget watchdog are forbidden.
