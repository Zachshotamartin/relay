#![forbid(unsafe_code)]
//! Typed client shell. Connection management and RWP/1 request correlation are
//! owned by R6. Server, WAL, and Raft implementation dependencies are forbidden;
//! async runtime use is confined to a convenience network adapter.
