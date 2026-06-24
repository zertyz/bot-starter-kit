//! `heed`-specific helpers & wrappers for async operation over LMDB's mmap-backed storage.
//!
//! `heed` reads can borrow bytes directly from LMDB's memory map. That is a stronger fit for
//! session-like bot state than a userspace database cache, as long as read transactions remain
//! short-lived.

mod async_heed;
pub use async_heed::*;

pub mod benchmark;
pub mod serde;
pub use serde::HeedPod;
