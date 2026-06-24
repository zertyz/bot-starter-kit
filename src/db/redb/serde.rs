//! Highly optimized serde options to enable zero-copy with `redb`

/// IMPORTANT: Only use this macro -- supporting the super optimized mmap SerDe strategy --
///            if you want to squeeze the last bit of performance possible.
///            Even so, the gains may be marginal when compared to the I/O cost -- specially for huge records.
///            Please use [RedbRkyvWrapper] instead -- which is several times safer.
///
/// That being said, apply this macro to your data model so it can be used as table records
/// for the `redb` engine -- SerDe will be done using mmap and zero-copy for reads and writes.
///
/// Please keep in mind the `mmap` limitations -- not all types support it:
///   - Only for fixed record sizes
///   - No pointers nor references -- specially, nothing that uses `Arc`, `Box`, and the like.
///   - All subtypes must adhere to the above constraints.
///   - Your type should be trivially copiable -- implementing the `Copy` trait is enforced.
///   - Not stable across different architectures (little endian / big-endian; 32 / 64 bits; etc).
///   - Might even not be stable across different compiler versions.
///   - Only use this macro if you also use database dumping functionalities -- specially,
///     dumping your data before replacing the executable with a new version is highly encouraged.
///   - If you this macro, please write automated tests to ensure everything keeps working
///     as code evolves and crate versions change -- or else you might lose your production data.
///   - The recommended approach is:
///     * write your `repository` tests
///     * take note of the database SHA256 digest
///     * include comparing the digest in a final assertion
///     * make sure the same tests will run on the production machine
///       -- or on a machine with the same Architecture, OS, Rust Compiler, ...
///
/// As a final note, please be aware that if your type (or subtypes) contain enums with associated data
/// -- and, due to this, bytes are wasted for many variants -- using this SerDe might not make sense at all:
///   - it will, most likely, require more disk space
///   - it might perform worse than [RedbRkyvWrapper].
#[macro_export]
macro_rules! redb_mmap_value {
    ($t:ty) => {
        impl redb::Value for $t
        where
            $t: Copy + bytemuck::Pod,
        {
            type SelfType<'a> = &'a $t;
            type AsBytes<'a>
                = &'a [u8]
            where
                Self: 'a;

            #[inline(always)]
            fn fixed_width() -> Option<usize> {
                Some(std::mem::size_of::<$t>())
            }

            #[inline(always)]
            fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
            where
                Self: 'a,
            {
                // unsafe { &*(data.as_ptr() as *const $t) }
                let ptr: *const $t = data.as_ptr().cast();
                unsafe { ptr.as_ref().unwrap_unchecked() }
            }

            #[inline(always)]
            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
            where
                Self: 'a,
            {
                unsafe {
                    let ptr = (*value as *const $t) as *const u8;
                    core::slice::from_raw_parts(ptr, core::mem::size_of::<$t>())
                }
            }

            #[inline(always)]
            fn type_name() -> redb::TypeName {
                redb::TypeName::new(concat!(stringify!($t), "_mmap"))
            }
        }
    };
}

/// Use this macro instead if you require more than 8 bytes alignment
/// (if your type has u128, for instance).
/// You lose Zero-Copy on reads.
#[macro_export]
macro_rules! redb_mmap_unaligned_value {
    ($t:ty) => {
        impl redb::Value for $t
        where
            $t: Copy + bytemuck::Pod,
        {
            type SelfType<'a> = $t;
            type AsBytes<'a>
                = &'a [u8]
            where
                Self: 'a;

            #[inline(always)]
            fn fixed_width() -> Option<usize> {
                Some(std::mem::size_of::<$t>())
            }

            #[inline(always)]
            fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
            where
                Self: 'a,
            {
                debug_assert_eq!(data.len(), core::mem::size_of::<$t>());
                unsafe { (data.as_ptr() as *const $t).read_unaligned() }
            }

            #[inline(always)]
            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
            where
                Self: 'a,
            {
                unsafe {
                    let ptr = (value as *const $t) as *const u8;
                    core::slice::from_raw_parts(ptr, core::mem::size_of::<$t>())
                }
            }

            #[inline(always)]
            fn type_name() -> redb::TypeName {
                redb::TypeName::new(concat!(stringify!($t), "_mmap"))
            }
        }
    };
}

//////////////////////////////////////////////////////////////////////
///////////////////////////////// RKYV ///////////////////////////////
//////////////////////////////////////////////////////////////////////

use redb::Value;
use rkyv::api::high::HighSerializer;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use rkyv::{Archive, Archived};
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::ops::Deref;

/// `Value` type for `redb` tables that uses `rkyv` for SerDe
/// with a tiny cost to serialize (on writes), but zero-copy to deserialize (on reads).
///
/// The costs are the allocation and serialization, where the copy of information occurs.
///
/// In exchange for this tiny cost, we have much more flexibility in the types we can
/// represent and in the size of the record -- which can be variable.
///
/// These are trade-offs when compared to the mmap SerDe offered by the [redb_mmap_value!()] macro.
#[derive(Clone)]
pub struct RedbRkyvWrapper<'a, T: Archive> {
    bytes: RkyvCow<'a>,
    phantom: std::marker::PhantomData<T>,
}
impl<'a, T: Archive> RedbRkyvWrapper<'a, T> {
    #[inline(always)]
    pub fn from_bytes_ref(bytes: &'a [u8]) -> Self {
        RedbRkyvWrapper {
            bytes: RkyvCow::Borrowed(bytes),
            phantom: PhantomData,
        }
    }

    #[inline(always)]
    pub fn from_bytes_owned(bytes: AlignedVec) -> Self {
        Self {
            bytes: RkyvCow::Owned(bytes),
            phantom: PhantomData,
        }
    }

    #[inline(always)]
    pub fn from_value(value: &T) -> Self
    where
        T: for<'r> rkyv::Serialize<
                HighSerializer<AlignedVec, ArenaHandle<'r>, rkyv::rancor::Error>,
            >,
    {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(value)
            .expect("rkyv serialization should not fail");
        Self::from_bytes_owned(bytes)
    }
}

impl<T: Archive + Debug> Value for RedbRkyvWrapper<'_, T>
where
    <T as Archive>::Archived: Debug,
{
    type SelfType<'a>
        = RedbRkyvWrapper<'a, T>
    where
        Self: 'a;
    type AsBytes<'a>
        = &'a [u8]
    where
        Self: 'a;

    #[inline(always)]
    fn fixed_width() -> Option<usize> {
        None
    }

    #[inline(always)]
    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        RedbRkyvWrapper::from_bytes_ref(data)
    }

    #[inline(always)]
    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        value.bytes.as_ref()
    }

    #[inline(always)]
    fn type_name() -> redb::TypeName {
        redb::TypeName::new(&format!("{}_rkyv", type_of::<T>()))
    }
}

#[inline(always)]
fn type_of<T>() -> &'static str {
    std::any::type_name::<T>()
}

impl<'a, T: Archive> Deref for RedbRkyvWrapper<'a, T> {
    type Target = Archived<T>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { rkyv::access_unchecked::<Archived<T>>(self.bytes.as_ref()) }
    }
}
impl<T: Archive + Debug> Debug for RedbRkyvWrapper<'_, T>
where
    <T as Archive>::Archived: Debug,
{
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <T::Archived as Debug>::fmt(self, f)
    }
}

impl<T: Archive + Debug> From<T> for RedbRkyvWrapper<'_, T>
where
    <T as Archive>::Archived: Debug,
    T: for<'r> rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'r>, rkyv::rancor::Error>>,
{
    #[inline(always)]
    fn from(value: T) -> Self {
        RedbRkyvWrapper::from_value(&value)
    }
}

#[derive(Clone)]
enum RkyvCow<'a> {
    Borrowed(&'a [u8]),
    Owned(AlignedVec),
}
impl AsRef<[u8]> for RkyvCow<'_> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        match self {
            RkyvCow::Borrowed(borrowed) => borrowed,
            RkyvCow::Owned(owned) => owned,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commons::test_commons::redb_test_commons::TestModel;
    use redb::{Database, ReadableDatabase, TableDefinition};

    #[test]
    fn test_mmap() {
        const TABLE: TableDefinition<&str, TestModel> = TableDefinition::new("test_mmap_data");

        let expected_data_iter = || {
            (0..128).map(|i| TestModel {
                count: 127 - i,
                whatever: i,
            })
        };

        let key = |i| format!("my_key{i}");

        let file_path = "/tmp/test_mmap.redb";
        let db = Database::create(file_path).expect("Could not create database");
        let write_txn = db.begin_write().expect("Could not begin write transaction");
        {
            let mut table = write_txn
                .open_table(TABLE)
                .expect("Could not open/create table");
            for (i, value) in expected_data_iter().enumerate() {
                table
                    .insert(key(i).as_str(), &value)
                    .expect("Could not insert/replace #{i}");
            }
        }
        write_txn.commit().expect("Could not commit transaction");

        let read_txn = db.begin_read().expect("Could not begin read transaction");
        let table = read_txn.open_table(TABLE).expect("Could not open table");
        for (i, expected_value) in expected_data_iter().enumerate() {
            let observed_value = table
                .get(key(i).as_str())
                .expect("Could not get {key(i)} from table")
                .unwrap();
            assert_eq!(observed_value.value(), &expected_value, "Failed at #{i}");
        }
    }

    #[test]
    fn test_rkyv() {
        const TABLE: TableDefinition<&str, RedbRkyvWrapper<TestModel>> =
            TableDefinition::new("test_rkyv_data");

        let expected_data_iter = || {
            (0..128).map(|i| TestModel {
                count: 127 - i,
                whatever: i,
            })
        };

        let key = |i| format!("my_key{i}");

        let file_path = "/tmp/test_rkyv.redb";
        let db = Database::create(file_path).expect("Could not create database");
        let write_txn = db.begin_write().expect("Could not begin write transaction");
        {
            let mut table = write_txn
                .open_table(TABLE)
                .expect("Could not open/create table");
            for (i, value) in expected_data_iter().enumerate() {
                table
                    .insert(key(i).as_str(), RedbRkyvWrapper::from(value))
                    .expect("Could not insert/replace #{i}");
            }
        }
        write_txn.commit().expect("Could not commit transaction");

        let read_txn = db.begin_read().expect("Could not begin read transaction");
        let table = read_txn.open_table(TABLE).expect("Could not open table");
        for (i, expected_value) in expected_data_iter().enumerate() {
            let observed_value = table
                .get(key(i).as_str())
                .expect("Could not get {key(i)} from table")
                .unwrap();
            assert_eq!(*observed_value.value(), expected_value, "Failed at #{i}");
        }
    }
}
