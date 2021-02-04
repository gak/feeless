use crate::cookie::Cookie;
use crate::state::State;
use crate::wire::Wire;
use feeless::{Public, Signature};
use std::convert::TryFrom;
use zerocopy::{AsBytes, FromBytes, Unaligned};

#[derive(Debug, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct NodeIdHandshakeQuery(pub Cookie);

impl<'a> NodeIdHandshakeQuery {
    const LEN: usize = Cookie::LEN;

    pub fn new(cookie: Cookie) -> Self {
        Self(cookie)
    }

    pub fn cookie(&self) -> &Cookie {
        &self.0
    }
}

impl Wire for NodeIdHandshakeQuery {
    fn serialize(&self) -> Vec<u8> {
        self.0.serialize()
    }

    fn deserialize(state: &State, data: &[u8]) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        let cookie = Cookie::deserialize(state, data)?;
        Ok(NodeIdHandshakeQuery(cookie))
    }

    fn len() -> usize {
        Self::LEN
    }
}

#[derive(Debug)]
pub struct NodeIdHandshakeResponse {
    pub public: Public,
    pub signature: Signature,
}

impl NodeIdHandshakeResponse {
    pub const LEN: usize = Public::LEN + Signature::LEN;

    pub fn new(public: Public, signature: Signature) -> Self {
        Self { public, signature }
    }
}

impl Wire for NodeIdHandshakeResponse {
    fn serialize(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(Self::LEN);
        v.extend_from_slice(&self.public.as_bytes());
        v.extend_from_slice(&self.signature.as_bytes());
        v
    }

    fn deserialize(_: &State, data: &[u8]) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        Ok(Self {
            public: Public::try_from(&data[0..Public::LEN])?,
            signature: Signature::try_from(&data[Public::LEN..])?,
        })
    }

    fn len() -> usize {
        Self::LEN
    }
}
