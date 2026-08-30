#![forbid(unsafe_code)]
//! Relay server shell. Production adapters and process wiring are owned by later
//! gates. Simulator, model, client, and CLI dependencies are forbidden, as are
//! direct clock or randomness calls outside the production environment adapter.
