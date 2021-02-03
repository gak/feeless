use anyhow::anyhow;
use std::convert::TryFrom;

#[derive(Debug)]
pub struct Signature([u8; Signature::LENGTH]);

impl Signature {
    pub const LENGTH: usize = 64;
}

impl TryFrom<&[u8]> for Signature {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != Self::LENGTH {
            return Err(anyhow!(
                "Invalid length: {}, expecting: {}",
                value.len(),
                Self::LENGTH
            ));
        }

        let mut s = Signature([0u8; Self::LENGTH]);
        s.0.copy_from_slice(value);
        Ok(s)
    }
}
