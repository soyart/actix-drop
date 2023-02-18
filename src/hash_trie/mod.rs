#![allow(dead_code)]

pub mod trie;

use std::collections::HashMap;
use std::sync::Mutex;

use trie::{SearchMode, Trie};

pub struct HashTrie {
    // trie stores hash as a trie tree. Full SHA2 hash is inserted
    // to the trie, and users will need to have at least N bytes of
    // hash prefix (stored in key_lengths) in order to access the values.
    trie: Mutex<Trie<u8, Vec<u8>>>,
    // key_lengths maps 4-byte prefix to minimum key needed
    // to access clipboard of this prefix.
    key_lengths: Mutex<HashMap<[u8; 4], usize>>,
}

impl HashTrie {
    fn new() -> Self {
        Self {
            trie: Trie::new().into(),
            key_lengths: HashMap::new().into(),
        }
    }

    /// insert inserts new hash into the trie, returning the index at which
    /// the hash is unique.
    fn insert<T: AsRef<[u8]>>(&mut self, hash: T) -> usize {
        let hash = hash.as_ref();
        let len = hash.len();

        let mut trie = self.trie.lock().unwrap();
        trie.insert(hash, hash.to_owned());

        let mut i = 0;
        let idx = loop {
            if i == len {
                break i;
            }

            if !trie.search(SearchMode::Prefix, &hash[..i]) {
                break i;
            }

            i += 1;
        };

        self.key_lengths.lock().unwrap().insert(
            hash[..4]
                .try_into()
                .expect("failed to convert hash to [u8; 4]"),
            idx,
        );

        idx
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_wrapper() {
        use super::*;
        let mut w = HashTrie::new();

        assert_eq!(w.insert("1234"), 4);
        assert_eq!(w.insert("2345"), 4);
        assert_eq!(w.insert("0000"), 4);
        assert_eq!(w.insert("12345"), 5);
        assert_eq!(w.insert("123456"), 6);
    }
}
