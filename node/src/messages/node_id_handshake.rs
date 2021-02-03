use crate::connection::Connection;
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
    const LENGTH: usize = Cookie::LENGTH;

    pub fn new(cookie: Cookie) -> Self {
        Self(cookie)
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
        Self::LENGTH
    }
}

#[derive(Debug)]
pub struct NodeIdHandshakeResponse {
    pub public: Public,
    pub signature: Signature,
}

impl NodeIdHandshakeResponse {
    pub const LENGTH: usize = Public::LENGTH + Signature::LENGTH;
}

impl Wire for NodeIdHandshakeResponse {
    fn serialize(&self) -> Vec<u8> {
        unimplemented!()
    }

    fn deserialize(_: &State, data: &[u8]) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        Ok(Self {
            public: Public::try_from(&data[0..Public::LENGTH])?,
            signature: Signature::try_from(&data[Public::LENGTH..])?,
        })
    }

    fn len() -> usize {
        Self::LENGTH
    }
}
