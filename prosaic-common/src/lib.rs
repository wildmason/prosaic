//! Shared type metadata for the prosaic template engine.
//!
//! Owns `ValueType`, the `PIPE_SPECS` registry, and const-eval helpers used
//! by both `prosaic-core` (at runtime) and `prosaic-derive` (at macro
//! expansion time). This crate is `no_std` and has no dependencies so it
//! can be included anywhere the other two crates run.

#![no_std]
