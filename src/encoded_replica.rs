use sha2::{Digest, Sha256};

use crate::*;

pub type Checksum = Vec<u8>;

/// A [`Replica`] encoded into a compact binary format suitable for
/// transmission over the network.
///
/// This struct is created by [`encode`](Replica::encode)ing a [`Replica`] and
/// can be decoded back into a [`Replica`] by calling
/// [`decode`](Replica::decode). See the documentation of those methods for
/// more information.
#[cfg_attr(docsrs, doc(cfg(feature = "encode")))]
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EncodedReplica {
    protocol_version: ProtocolVersion,
    checksum: Checksum,
    bytes: Vec<u8>,
}

impl EncodedReplica {
    #[inline]
    pub(crate) fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    #[inline]
    pub(crate) fn checksum(&self) -> &Checksum {
        &self.checksum
    }

    #[inline]
    pub(crate) fn new(
        protocol_version: ProtocolVersion,
        checksum: Checksum,
        bytes: Vec<u8>,
    ) -> Self {
        Self { protocol_version, checksum, bytes }
    }

    #[inline]
    pub(crate) fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }
}

/// The type of error that can occur when [`decode`](Replica::decode)ing an
/// [`EncodedReplica`].
#[cfg_attr(docsrs, doc(cfg(feature = "encode")))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// This error occurs when the internal checksum of the [`EncodedReplica`]
    /// fails.
    ///
    /// This typically means that the [`EncodedReplica`] was corrupted during
    /// transmission.
    ChecksumFailed,

    /// This error occurs when the machine that created the [`EncodedReplica`]
    /// and the one that is trying to [`decode`](Replica::decode) it are using
    /// two incompatible versions of cola.
    DifferentProtocol {
        /// The `ProtocolVersion` of cola on the machine that created the
        /// `EncodedReplica`.
        encoded_on: ProtocolVersion,

        /// The `ProtocolVersion` of cola on the machine that is trying to
        /// decode the `EncodedReplica`.
        decoding_on: ProtocolVersion,
    },

    /// This error is an umbrella variant that encompasses all other errors
    /// that can occur when the binary data wrapped by the [`EncodedReplica`]
    /// cannot be decoded into a `Replica`.
    ///
    /// This is returned when the checksum and protocol version checks both
    /// succeed, *and yet* the data is still invalid. The only way this can
    /// occur in practice is if the `EncodedReplica` passed to
    /// [`decode`](Replica::decode) was deserialized from a byte vector that
    /// was not the result of serializing an `EncodedReplica`.
    ///
    /// As long as you're not doing that (and you shouldn't be) this variant
    /// can be ignored.
    InvalidData,
}

#[inline]
pub fn checksum(bytes: &[u8]) -> Checksum {
    Sha256::digest(bytes)[..].to_vec()
}
