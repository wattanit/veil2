//! Serialising fixed-size byte arrays as CBOR byte strings.
//!
//! `serde` only derives array impls up to length 32, and the entry model
//! carries a 48-byte wrapped key and a 19-byte nonce prefix. Encoding them as
//! byte strings rather than arrays of numbers also keeps the index compact:
//! at C-1's 65,536 entries the difference is measured in megabytes.

use serde::de::{Error as _, SeqAccess, Visitor};
use serde::{Deserializer, Serializer};

/// Serialises a fixed-size array as a CBOR byte string.
///
/// # Errors
///
/// Propagates the serialiser's own failure.
pub fn serialize<S: Serializer, const N: usize>(
    value: &[u8; N],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(value)
}

/// Deserialises a fixed-size array from a CBOR byte string or sequence.
///
/// A length other than `N` is an error rather than a truncation or a pad: a
/// wrapped key that is the wrong length is a damaged index, not a shorter key.
///
/// # Errors
///
/// Fails when the input is not bytes, or is bytes of the wrong length.
pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
    deserializer: D,
) -> Result<[u8; N], D::Error> {
    struct ArrayVisitor<const N: usize>;

    impl<'de, const N: usize> Visitor<'de> for ArrayVisitor<N> {
        type Value = [u8; N];

        fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{N} bytes")
        }

        fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
            v.try_into().map_err(|_| E::invalid_length(v.len(), &self))
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut out = [0u8; N];
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::invalid_length(i, &self))?;
            }
            if seq.next_element::<u8>()?.is_some() {
                return Err(A::Error::invalid_length(N + 1, &self));
            }
            Ok(out)
        }
    }

    deserializer.deserialize_bytes(ArrayVisitor::<N>)
}
