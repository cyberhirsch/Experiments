//! № 18 — Galaxy Colonization Simulator.
//!
//! The model and GPU layers live here so that the binary, the headless
//! `limits` probe and the tests all share one definition of the galaxy
//! rather than three that can drift apart.

pub mod colonize;
pub mod galaxy;
pub mod gpu;
