//! Common code for redb tests

use crate::redb_mmap_value;

/// Model used by redb tests
// annotations for mmap
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
// annotations for `rkyv`
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq))]
// common recommended annotations
#[derive(Debug, PartialEq)]
pub struct TestModel {
    pub count: u64,
    pub whatever: u64,
    // pub whatever: u128,  // if you add a `u128` field, `mmap_unaligned_value!()` must be used instead of `mmap_value!()`
    // (zero-copy on reads will be gone)
}

// for mmap, we must apply one of the mmap macros to the model
// for `rkyv`, there is no need of taking any additional actions

redb_mmap_value!(TestModel); // zero-copy for reads & writes, but works only with 8-byte aligned fields

// mmap_unaligned_value!(MyModel);     // this is used if your field requires >8 alignment
// (if you use u128). Zero-Copy on reads are gone.
