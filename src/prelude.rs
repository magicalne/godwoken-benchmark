use bytes::Bytes;
pub use molecule::prelude::{Builder, Entity, Reader};

use crate::generated::packed;

pub trait Unpack<T> {
    fn unpack(&self) -> T;
}

pub trait Pack<T: Entity> {
    fn pack(&self) -> T;
}

pub trait PackVec<T: Entity, I: Entity>: IntoIterator<Item = I> {
    fn pack(self) -> T;
}

/// An alias of `from_slice(..)` to mark where we are really have confidence to do unwrap on the result of `from_slice(..)`.
pub trait FromSliceShouldBeOk<'r>: Reader<'r> {
    /// Unwraps the result of `from_slice(..)` with confidence and we assume that it's impossible to fail.
    fn from_slice_should_be_ok(slice: &'r [u8]) -> Self;
}

impl<'r, R> FromSliceShouldBeOk<'r> for R
where
    R: Reader<'r>,
{
    fn from_slice_should_be_ok(slice: &'r [u8]) -> Self {
        match Self::from_slice(slice) {
            Ok(ret) => ret,
            Err(_err) => panic!("invalid molecule structure"),
        }
    }
}

impl Pack<packed::Bytes> for Bytes {
    fn pack(&self) -> packed::Bytes {
        let len = self.len();
        let mut vec: Vec<u8> = Vec::with_capacity(4 + len);
        vec.extend_from_slice(&(len as u32).to_le_bytes()[..]);
        vec.extend_from_slice(self);
        packed::Bytes::new_unchecked(Bytes::from(vec))
    }
}

impl<'r> Unpack<Bytes> for packed::BytesReader<'r> {
    fn unpack(&self) -> Bytes {
        Bytes::from(self.raw_data().to_vec())
    }
}

impl Unpack<Bytes> for packed::Bytes {
    fn unpack(&self) -> Bytes {
        self.raw_data()
    }
}
