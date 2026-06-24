//! This module is the substitute for the raw `redb` crate bringing in the following features:
//!   - Seamless support for custom Rust types
//!     - truly zero-copy with mmap (the fastest, but least flexible & least portable serde option -- only suitable for fixed size records)
//!     - zero-copy on reads with `rkyv` (same read speeds but slower on writes -- the trade-off for allowing flexible & complex types & variable sized records)
//!   - Seamless data export/import functionalities
//!   - Seamless database maintenance
//!
//! Your application should not use `redb` directly, but this module instead, meaning there
//! should be no occurrences of "redb::" in your source code.

pub mod redb_repository;
pub mod serde;
