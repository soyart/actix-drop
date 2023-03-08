use std::sync::Mutex;

use soytrie::Trie;
use tokio::sync::oneshot;

use super::clipboard::Clipboard;
use super::error::StoreError;
use super::persist;

// Tracker replacement, with trie from soytrie.
pub(super) struct TrieTracker {
    // trie keeps tuple (full_hash, clipboard, aborter) as value
    trie: Mutex<Trie<u8, (String, Option<Clipboard>, oneshot::Sender<()>)>>,
}

impl TrieTracker {
    const MIN_HASH_LEN: usize = 4;

    pub fn new() -> Self {
        Self {
            trie: Trie::new().into(),
        }
    }

    pub fn remove(
        &self,
        hash: &str,
    ) -> Option<(String, Option<Clipboard>, oneshot::Sender<()>)> {
        self.trie
            .lock()
            .expect("failed to lock trie")
            .remove(hash.as_bytes())
            .and_then(|node| node.value)
    }

    pub fn is_empty(&self) -> bool {
        self.trie
            .lock()
            .expect("failed to lock trie")
            .all_valued_children()
            .len()
            == 0
    }

    // Reports whether there's only 1 child below frag
    // Note: is only here for testing
    #[allow(unused)]
    pub fn is_shortest_ref(&self, frag: &str) -> bool {
        self.trie
            .lock()
            .expect("failed to lock trie")
            .predict(frag.as_bytes())
            .is_some_and(|targets| targets.len() == 1)
    }

    // Returns the clipboard if it's the only child below frag
    pub fn get_clipboard_frag(&self, frag: &str) -> Option<Clipboard> {
        match self
            .trie
            .lock()
            .expect("failed to lock trie")
            .get_child(frag.as_ref())
            .and_then(|trie| Some(trie.all_valued_children()))
        {
            None => None,
            Some(children) if children.len() != 1 => None,

            Some(child) => {
                match child[0].value {
                    None => None,
                    // Clipboard::Mem
                    Some((_, Some(ref mem_clip), _)) => Some(mem_clip.to_owned()),
                    // Clipboard::Persist
                    Some((ref hash, None, _)) => match persist::read_clipboard_file(&hash) {
                        Err(_) => None,
                        Ok(persisted) => Some(Clipboard::Persist(persisted.into())),
                    },
                }
            }
        }
    }

    // Returns the clipboard at hash (supposedly the end of trie)
    #[allow(unused)]
    pub fn get_clipboard(&self, hash: &str) -> Option<Clipboard> {
        self.trie
            .lock()
            .expect("failed to lock trie")
            .get_child(hash.as_ref())
            .and_then(|child| match child.value {
                // No clipboard registered
                None => None,
                // Clipboard::Mem
                Some((_, Some(ref mem_clip), _)) => Some(mem_clip.to_owned()),
                // Clipboard::Persist
                Some(_) => match persist::read_clipboard_file(hash) {
                    Err(_) => None,
                    Ok(persisted) => Some(Clipboard::Persist(persisted.into())),
                },
            })
    }

    // Inserts clipboard to the tracker. If the clipboard is duplicate (i.e. the hash collides),
    // aborts the timer previously set and start a new one. It returns the minimum length of hash (frag)
    // required to uniquely access this clipboard. `insert_clipboard` expects that the hashes are of
    // uniform, constant length.
    pub fn insert_clipboard(
        &self,
        hash: &str,
        clipboard: Clipboard,
        // The usize returned is the shortest hash length for which the hash
        // can still be uniquely accessed.
    ) -> Result<(usize, oneshot::Receiver<()>), StoreError> {
        let mut trie = self.trie.lock().unwrap();

        // Abort previous timer, and delete persisted file if there's one
        trie.remove(hash.as_ref())
            .and_then(|target_child| target_child.value)
            .map(|value| {
                // Clipboard::Mem has None stored
                if value.1.is_none() {
                    persist::rm_clipboard_file(hash)?;
                }

                Ok::<tokio::sync::oneshot::Sender<()>, StoreError>(value.2)
            })
            .transpose()?
            .map(|aborter| match aborter.send(()) {
                Err(_) => Err(StoreError::Bug(format!(
                    "failed to abort prev timer: {hash}",
                ))),

                _ => Ok(()),
            })
            .transpose()?;

        // Find the return value min_len (default is 4, MIN_HASH_LEN).
        // If the first 4 chars of |hash| is not in the trie,
        // then we call it a day and assign idx to 4.
        // If there's a child matching the fist 4 hash chars,
        // then we traverse from 4..hash_len until we reach a point
        // where a child is child-less and assign idx to.
        let mut min_len = Self::MIN_HASH_LEN;

        match trie
            .as_ref()
            .get_child(&hash[..Self::MIN_HASH_LEN].as_ref())
        {
            None => min_len = Self::MIN_HASH_LEN,
            Some(mut curr) => {
                let h: &[u8] = hash.as_ref();

                for i in (Self::MIN_HASH_LEN..hash.len()).into_iter() {
                    match curr.get_direct_child(h[i]) {
                        None => {
                            min_len = i + 1;
                            break;
                        }
                        Some(next) => {
                            curr = next;
                        }
                    }
                }
            }
        }

        let to_save = match clipboard.clone() {
            // Clipboard::Mem(data) => data will have to live in haystack
            clip @ Clipboard::Mem(_) => Some(clip),

            // Clipboard::Persist(data) => data does not have to live in haystack
            Clipboard::Persist(data) => {
                persist::write_clipboard_file(hash, data.as_ref())?;
                None
            }
        };

        let (tx, rx) = oneshot::channel();
        trie.insert_value(hash.as_ref(), (hash.to_string(), to_save, tx));

        Ok((min_len, rx))
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn insert() {
        use super::*;
        let vals = vec![
            ("123400000", 4), // Accessing this node requires designed minimum length 4
            ("123450000", 5), // Accessing this now requires 5 characters
            ("123456780", 6), // and so on..
            ("abcd1234x", 4),
            ("abcd12345", 9), // We need to go all the way "down" to character '5' to get unique value
            ("abcd00000", 5), // Here we only need 5 characters ("abcd0") to distinguish it from other nodes with "abcd"*
            ("abcdx0000", 5), // And this entry would need 5 ("abcdx")
        ];

        let htrie = TrieTracker::new();
        vals.into_iter().enumerate().for_each(|(idx, val)| {
            println!("{} {v}", idx + 1, v = val.0);
            assert_eq!(
                htrie
                    .insert_clipboard(val.0.as_ref(), Clipboard::Mem(val.0.into()))
                    .expect("failed to insert {val}")
                    .0,
                // Expected return value (min hash len)
                val.1,
            );
        });
    }
}
