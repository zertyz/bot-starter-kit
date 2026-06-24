//! Codecs for `heed`.
//!
//! LMDB gives us borrowed bytes from the mmap, but those bytes are not a portable `&T` unless
//! their address satisfies `T`'s alignment. This module keeps the borrowed bytes truly zero-copy
//! and makes typed access explicit.

use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem::size_of;

use ::heed::{BoxedError, BytesDecode, BytesEncode};
use rkyv::api::high::HighSerializer;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use rkyv::{Archive, Archived};

/// Fixed-size POD codec for `heed`.
///
/// Writes are borrowed raw bytes. Reads return [`HeedPodRef`], which is a borrowed view over the
/// LMDB mmap bytes. Use [`HeedPodRef::try_as_aligned`] only when you really need a typed reference;
/// otherwise use [`HeedPodRef::as_bytes`] or an explicit unaligned load.
///
/// This is a hot-path format, not a portable persistence format: it inherits Rust layout,
/// endianness, padding, and compiler/version caveats from the concrete POD type.
pub struct HeedPod<T>(PhantomData<T>);

#[derive(Clone, Copy)]
pub struct HeedPodRef<'a, T> {
    bytes: &'a [u8],
    phantom: PhantomData<T>,
}

impl<'a, T> HeedPodRef<'a, T> {
    #[inline(always)]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[inline(always)]
    pub fn try_as_aligned(&self) -> Result<&'a T, bytemuck::PodCastError>
    where
        T: bytemuck::Pod,
    {
        bytemuck::try_from_bytes(self.bytes)
    }

    #[inline(always)]
    pub fn read_unaligned(&self) -> T
    where
        T: bytemuck::Pod,
    {
        bytemuck::pod_read_unaligned(self.bytes)
    }
}

impl<'a, T> BytesEncode<'a> for HeedPod<T>
where
    T: bytemuck::Pod + 'a,
{
    type EItem = T;

    #[inline(always)]
    fn bytes_encode(item: &'a Self::EItem) -> Result<Cow<'a, [u8]>, BoxedError> {
        Ok(Cow::Borrowed(bytemuck::bytes_of(item)))
    }
}

impl<'a, T> BytesDecode<'a> for HeedPod<T>
where
    T: bytemuck::Pod + 'a,
{
    type DItem = HeedPodRef<'a, T>;

    #[inline(always)]
    fn bytes_decode(bytes: &'a [u8]) -> Result<Self::DItem, BoxedError> {
        let expected_len = size_of::<T>();
        if bytes.len() != expected_len {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "invalid POD value width: got {} bytes, expected {expected_len}",
                    bytes.len()
                ),
            )));
        }

        Ok(HeedPodRef {
            bytes,
            phantom: PhantomData,
        })
    }
}

/// Rkyv codec for variable-sized values.
///
/// The read path returns an archived value borrowed from LMDB's mmap. The write path still
/// serializes into an `AlignedVec` and then copies into `Vec<u8>` because `heed`'s general
/// `BytesEncode` API returns `Cow<[u8]>`. For write-heavy hot paths, prefer `put_reserved` with
/// a custom writer once a repository abstraction owns the operation.
pub struct HeedRkyv<T>(PhantomData<T>);

impl<'a, T> BytesEncode<'a> for HeedRkyv<T>
where
    T: Archive
        + for<'r> rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'r>, rkyv::rancor::Error>>
        + 'a,
{
    type EItem = T;

    #[inline(always)]
    fn bytes_encode(item: &'a Self::EItem) -> Result<Cow<'a, [u8]>, BoxedError> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(item).map_err(|err| {
            Box::new(std::io::Error::other(format!(
                "rkyv serialization failed: {err:?}"
            ))) as BoxedError
        })?;
        Ok(Cow::Owned(bytes.into_vec()))
    }
}

impl<'a, T> BytesDecode<'a> for HeedRkyv<T>
where
    T: Archive + 'a,
    Archived<T>: rkyv::Portable,
{
    type DItem = &'a Archived<T>;

    #[inline(always)]
    fn bytes_decode(bytes: &'a [u8]) -> Result<Self::DItem, BoxedError> {
        // Safety invariant for callers of the unchecked rkyv API: this codec expects bytes that
        // were produced by the matching rkyv serializer, or validated at a repository boundary.
        Ok(unsafe { rkyv::access_unchecked::<Archived<T>>(bytes) })
    }
}
