use std::convert::TryFrom;

use crate::difficulty::Difficulty;
use crate::encoding::blake2b;
use crate::{expect_len, hex_formatter, BlockHash, Public};
use rand::RngCore;
use std::fmt::{Debug, Formatter};

#[derive(Debug)]
pub enum Subject {
    Hash(BlockHash),
    Public(Public),
}

impl Subject {
    pub fn to_bytes(&self) -> &[u8] {
        match self {
            Subject::Hash(h) => h.as_bytes(),
            Subject::Public(p) => p.as_bytes(),
        }
    }
}

pub struct Work([u8; Work::LEN]);

impl Work {
    pub const LEN: usize = 8;

    pub fn zero() -> Self {
        Self([0u8; Self::LEN])
    }

    pub fn random() -> Self {
        let mut s = Self([0u8; Self::LEN]);
        rand::thread_rng().fill_bytes(&mut s.0);
        s
    }

    pub fn from_hex(s: &str) -> anyhow::Result<Self> {
        let mut value = hex::decode(s)?;
        let value = value.as_slice();
        Work::try_from(value)
    }

    /// Block and generate forever until we find a solution.
    pub fn generate(subject: &Subject, threshold: &Difficulty) -> anyhow::Result<Work> {
        loop {
            let work = Work::attempt(&subject, &threshold)?;
            if work.is_none() {
                continue;
            }
            return Ok(work.unwrap());
        }
    }

    /// A single attempt.
    pub fn attempt(subject: &Subject, threshold: &Difficulty) -> anyhow::Result<Option<Work>> {
        let work = Work::random();
        if work.verify(subject, threshold)? {
            Ok(Some(work))
        } else {
            Ok(None)
        }
    }

    pub fn hash(work_and_subject: &[u8]) -> Box<[u8]> {
        blake2b(Self::LEN, work_and_subject)
    }

    pub fn verify(&self, subject: &Subject, threshold: &Difficulty) -> anyhow::Result<bool> {
        let difficulty = self.get_difficulty(subject)?;
        Ok(difficulty.is_more_than(threshold))
    }

    // This is very probably not performant, but I'm just here to make it work first.
    pub fn get_difficulty(&self, subject: &Subject) -> anyhow::Result<Difficulty> {
        let mut work_and_subject = Vec::new();

        // For some reason this is reversed!
        let mut reversed_work = self.0.to_vec();
        reversed_work.reverse();

        work_and_subject.extend_from_slice(&reversed_work);
        work_and_subject.extend_from_slice(subject.to_bytes());
        let mut hash = Self::hash(&work_and_subject);
        Difficulty::from_le_slice(hash.as_ref())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Work(")?;
        hex_formatter(f, &self.0)?;
        write!(f, ")")?;
        Ok(())
    }
}

impl TryFrom<&[u8]> for Work {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        expect_len(value.len(), Self::LEN, "Work")?;

        let mut s = Work::zero();
        s.0.copy_from_slice(value);
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;

    #[test]
    fn verify() {
        // Each hash is incremented by one.
        let fixtures = vec![
            (
                "2387767168f9453db0eca227c79d7e7a31b78cafb58bd9cdee630881c70979b8",
                "c3f097857cc7106b",
                "fffffff867b3146b",
                true,
            ),
            (
                "2387767168f9453db0eca227c79d7e7a31b78cafb58bd9cdee630881c70979b9",
                "ec4f0960a70fdcbe",
                "fffffffde26451db",
                true,
            ),
            (
                "2387767168f9453db0eca227c79d7e7a31b78cafb58bd9cdee630881c70979ba",
                "b58e13f297179bc2",
                "fffffffb6fc1b4a6",
                true,
            ),
            // This is the same as above except the work is just zeros,
            // causing a totally different difficulty, and not enough work in this case.
            (
                "2387767168f9453db0eca227c79d7e7a31b78cafb58bd9cdee630881c70979ba",
                "0000000000000000",
                "357abcab02726362",
                false,
            ),
        ];

        let threshold = Difficulty::from_hex("ffffffc000000000").unwrap();
        for fixture in fixtures {
            let (hash, work, expected_difficulty, is_enough_work) = &fixture;
            let hash = BlockHash::from_hex(hash).unwrap();
            let subject = Subject::Hash(hash);
            let work = Work::from_hex(work).unwrap();
            let expected_difficulty = Difficulty::from_hex(expected_difficulty).unwrap();
            let difficulty = work.get_difficulty(&subject).unwrap();
            assert_eq!(difficulty, expected_difficulty, "{:?}", &fixture);
            assert_eq!(
                work.verify(&subject, &threshold).unwrap(),
                *is_enough_work,
                "{:?}",
                &fixture
            );
        }
    }

    #[test]
    fn generate_work() {
        // Let's use a low difficulty in debug mode, it doesn't take forever.
        let threshold = if cfg!(debug_assertions) {
            Difficulty::from_hex("ffff000000000000")
        } else {
            Difficulty::from_hex("ffffffc000000000")
        }
        .unwrap();
        dbg!(&threshold);

        let public = Seed::zero().derive(0).to_public();
        dbg!(&public);
        let subject = Subject::Public(public);
        let work = Work::generate(&subject, &threshold).unwrap();
        dbg!(&work);
        assert!(work.verify(&subject, &threshold).unwrap());
    }
}