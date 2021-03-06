use crate::encoding::ALPHABET;
use crate::phrase::{Language, MnemonicType};
use crate::{Address, Phrase, Private, Seed};
use anyhow::anyhow;
use regex::Regex;
use std::sync::{Arc, RwLock};
use std::thread;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{info, trace};

#[derive(Clone)]
pub enum SecretType {
    Phrase {
        language: Language,
        words: MnemonicType,
    },
    Seed,
    Private,
}

#[derive(Debug)]
pub enum Secret {
    Phrase(Phrase),
    Seed(Seed),
    Private(Private),
}

#[derive(Debug)]
pub struct SecretResult {
    pub secret: Secret,
    pub address: Address,
}

impl SecretResult {
    fn new(secret: Secret, address: Address) -> Self {
        Self { secret, address }
    }
}

#[derive(Clone, Copy)]
enum SearchOffset {
    FirstDigit = 5,
    SkipFirstDigit = 6,
}

#[derive(Clone)]
pub struct Vanity {
    secret_type: SecretType,
    matches: Match,
    index: u32,
    tasks: Option<usize>,
    search_offset: SearchOffset,

    /// How many attempts to loop through before checking if the channel is closed.
    ///
    /// The bigger the number here, the slower it will be to gracefully quit when requested.
    check_count: usize,
}

impl Vanity {
    pub fn new(secret_type: SecretType, matches: Match) -> Self {
        Self {
            secret_type,
            matches,
            index: 0, // TODO: Make this a user option, maybe allow to scan up to N too.
            tasks: None,
            check_count: 10000,
            search_offset: SearchOffset::SkipFirstDigit,
        }
    }

    /// Number of tasks to spawn.
    pub fn tasks(&mut self, v: usize) -> &mut Vanity {
        self.tasks = Some(v);
        self
    }

    /// Should the search include the first number after `nano_` (1 or 3)?
    pub fn include_first_digit(&mut self, v: bool) -> &mut Vanity {
        self.search_offset = if v {
            SearchOffset::FirstDigit
        } else {
            SearchOffset::SkipFirstDigit
        };
        self
    }

    /// Spawn some tasks to try to find a vanity address.
    ///
    /// This returns a [Receiver] containing [SecretResult]s for each found address, and a
    /// [Arc] [RwLock] counter of attempts.
    pub async fn start(self) -> anyhow::Result<(Receiver<SecretResult>, Arc<RwLock<usize>>)> {
        self.validate()?;
        let cpus = num_cpus::get();
        let attempts = Arc::new(RwLock::new(0usize));
        let tasks = self.tasks.unwrap_or(cpus);
        let (tx, rx) = tokio::sync::mpsc::channel::<SecretResult>(100);
        info!("Starting {} vanity tasks", tasks);
        for _ in 0..tasks {
            let v = self.clone();
            let tx_ = tx.clone();
            let counter_ = attempts.clone();
            thread::spawn(move || {
                v.single_threaded_worker(tx_, counter_);
            });
        }
        Ok((rx, attempts))
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let s = match &self.matches {
            Match::StartOrEnd(s) => s,
            Match::Start(s) => s,
            Match::End(s) => s,
            // TODO: Extract literals from regexp, or just ignore regexp characters (.$^{}[] etc)
            Match::Regex(_) => return Ok(()),
        };
        let re = regex::Regex::new(&format!("^[{}]*$", ALPHABET)).unwrap();
        if re.is_match(s) {
            Ok(())
        } else {
            Err(anyhow!("Your search won't ever match because it has characters that aren't valid. Valid characters: {}", ALPHABET))
        }
    }

    fn single_threaded_worker(&self, tx: Sender<SecretResult>, counter: Arc<RwLock<usize>>) {
        while !tx.is_closed() {
            for _ in 0..self.check_count {
                if let Some(result) = self.single_attempt() {
                    if let Err(_) = tx.blocking_send(result) {
                        trace!("Exiting vanity task due to closed channel while sending.");
                        return;
                    }
                }
            }
            let mut c = counter.write().expect("Could not lock counter for writing");
            *c += self.check_count;
            drop(c);
        }
        trace!("Exiting vanity task due to closed channel.");
    }

    fn single_attempt(&self) -> Option<SecretResult> {
        let result = match &self.secret_type {
            SecretType::Seed => {
                let seed = Seed::random();
                // This should never panic because the public key comes from a legit private key.
                let address = seed.derive(self.index).to_address().unwrap();
                SecretResult::new(Secret::Seed(seed), address)
            }
            SecretType::Private => {
                let private = Private::random();
                // This should never panic because the public key comes from a legit private key.
                let address = private.to_address().unwrap();
                SecretResult::new(Secret::Private(private), address)
            }
            SecretType::Phrase { language, words } => {
                // This should never panic because the public key comes from a legit private key.
                let phrase = Phrase::random(words.to_owned(), language.to_owned());
                let address = phrase.to_private(0, "").unwrap().to_address().unwrap();
                SecretResult::new(Secret::Phrase(phrase), address)
            }
        };

        let addr = &result.address.to_string();
        let offset = self.search_offset as usize;
        let searchable = &addr[offset..];

        let good = match &self.matches {
            Match::StartOrEnd(s) => searchable.starts_with(s) || searchable.ends_with(s),
            Match::Start(s) => searchable.starts_with(s),
            Match::End(s) => searchable.ends_with(s),
            Match::Regex(re) => re.is_match(searchable),
        };

        if good {
            Some(result)
        } else {
            None
        }
    }

    /// Block until all results are collected up to a size of `limit`.
    pub async fn collect(self, mut limit: usize) -> anyhow::Result<Vec<SecretResult>> {
        let (mut rx, _) = self.start().await?;
        let mut collected = vec![];
        while let Some(result) = rx.recv().await {
            collected.push(result);
            limit -= 1;
            if limit == 0 {
                break;
            }
        }
        Ok(collected)
    }
}

#[derive(Clone)]
pub enum Match {
    StartOrEnd(String),
    Start(String),
    End(String),
    Regex(Regex),
}

impl Match {
    pub fn start_or_end(s: &str) -> Self {
        Match::StartOrEnd(s.into())
    }

    pub fn start(s: &str) -> Self {
        Match::Start(s.into())
    }

    pub fn end(s: &str) -> Self {
        Match::End(s.into())
    }

    pub fn regex(s: &str) -> anyhow::Result<Self> {
        let r = regex::Regex::new(s.into())?;
        Ok(Match::Regex(r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn vanitize_start_or_end() {
        let vanity = Vanity::new(SecretType::Seed, Match::start_or_end("g"));
        let limit = 20; // Should be enough for 1 in a million chance of this test failing.
        let results = vanity.collect(limit).await.unwrap();
        assert_eq!(results.len(), limit);
        let mut has_start = false;
        let mut has_end = false;
        for result in results {
            let addr = result.address.to_string();
            if addr[6..].starts_with("g") {
                has_start = true;
            }
            if addr.ends_with("g") {
                has_end = true;
            }
        }
        assert!(has_start);
        assert!(has_end);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vanitize_start() {
        let results = Vanity::new(SecretType::Seed, Match::start("z"))
            .collect(1)
            .await
            .unwrap();
        assert_eq!(&results[0].address.to_string()[6..7], "z");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vanitize_end() {
        let results = Vanity::new(SecretType::Seed, Match::end("z"))
            .collect(1)
            .await
            .unwrap();
        assert!(&results[0].address.to_string().ends_with("z"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vanitize_regex() {
        let results = Vanity::new(SecretType::Seed, Match::regex("z.*z.*z").unwrap())
            .collect(1)
            .await
            .unwrap();
        let addr = &results[0].address.to_string();
        dbg!(&addr);
        assert!(addr.matches("z").count() >= 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vanitize_private() {
        let results = Vanity::new(SecretType::Private, Match::end("zz"))
            .collect(1)
            .await
            .unwrap();
        let result = &results[0];

        let addr = &result.address.to_string();
        dbg!(&addr);
        assert!(addr.ends_with("zz"));
        if let Secret::Private(private) = &result.secret {
            assert_eq!(addr, &private.to_address().unwrap().to_string());
        } else {
            assert!(false, "Did not get a private key");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vanitize_first_digit() {
        let mut vanity = Vanity::new(SecretType::Private, Match::start("1z"));
        vanity.include_first_digit(true);
        let results = vanity.collect(1).await.unwrap();
        let result = &results[0];

        let addr = &result.address.to_string();
        dbg!(&addr);
        assert_eq!(&addr[5..7], "1z");
    }

    // Phrase is waaaay to slow to test.
    // #[tokio::test(flavor = "multi_thread")]
    // async fn vanitize_phrase() {
    //     let results = Vanity::new(
    //         SecretType::Phrase {
    //             language: Language::Japanese,
    //             words: MnemonicType::Words24,
    //         },
    //         Match::end("z"),
    //     )
    //     .collect(1)
    //     .await
    //     .unwrap();
    //     let result = &results[0];
    //
    //     let addr = &result.address.to_string();
    //     dbg!(&addr);
    //     assert!(addr.ends_with("z"));
    //     if let Secret::Phrase(phrase) = &result.secret {
    //         assert_eq!(
    //             addr,
    //             &phrase
    //                 .to_private(0, "")
    //                 .unwrap()
    //                 .to_address()
    //                 .unwrap()
    //                 .to_string()
    //         );
    //     } else {
    //         assert!(false, "Did not get a phrase");
    //     }
    // }

    #[test]
    fn validate() {
        let v = Vanity::new(SecretType::Private, Match::start("l"));
        assert!(v.validate().is_err());
    }
}
