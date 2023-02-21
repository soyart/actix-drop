#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use soytrie::{Trie, TrieNode};
use tokio::sync::oneshot;

use super::clipboard::Clipboard;
use super::error::StoreError;
use super::persist;

// Tracker replacement, with trie from soytrie.
pub struct TrieTracker {
    // trie keeps tuple (full_hash, clipboard, aborter) as value
    trie: Mutex<Trie<u8, (String, Option<Clipboard>, oneshot::Sender<()>)>>,
}

impl TrieTracker {
    const MIN_HASH_LEN: usize = 4;

    fn new() -> Self {
        Self {
            trie: Trie::new().into(),
        }
    }

    // Reports whether there's only 1 child below frag
    // Note: is only here for testing
    fn is_shortest_ref(&self, frag: &str) -> bool {
        self.trie
            .lock()
            .expect("failed to lock trie")
            .predict(frag.as_ref())
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
            Some(children) => {
                if children.len() > 1 {
                    return None;
                }

                match children[0].value {
                    None => None,
                    // Clipboard::Persist
                    Some((ref hash, None, _)) => match persist::read_clipboard_file(&hash) {
                        Err(_) => None,
                        Ok(persisted) => Some(Clipboard::Persist(persisted.into())),
                    },
                    // Clipboard::Mem
                    Some((_, Some(ref mem_clip), _)) => Some(mem_clip.to_owned()),
                }
            }
        }
    }

    // Returns the clipboard at hash (supposedly the end of trie)
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
}

// Inserts clipboard to the tracker. If the clipboard is duplicate (i.e. the hash collides),
// aborts the timer previously set and start a new one. It returns the minimum length of hash (frag)
// required to uniquely access this clipboard.
fn insert_clipboard(
    tracker: Arc<TrieTracker>,
    hash: &str,
    clipboard: Clipboard,
    dur: Duration,
    // The usize returned is the shortest hash length for which the hash
    // can still be uniquely accessed.
) -> Result<usize, StoreError> {
    let mut trie = tracker.trie.lock().unwrap();

    // Abort previous timer, and delete persisted file if there's one
    trie.remove(hash.as_ref())
        .and_then(|target_child| target_child.value)
        .map(|value| {
            // Clipboard::Mem has None stored
            if value.1.is_none() {
                persist::rm_clipboard_file(hash)?;
            }

            println!("debug: {} is killing {}", hash, value.0);

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

    let mut idx = 0;

    {
        // Find the idx return value
        //
        // We don't need mutable reference to the TrieNode because we are only reading
        // values out of it, although we need `curr` to be mutable so that we can't assign it a new
        // value. The real deletion is done on `trie`, which is mutable thanks to MutexGuard
        let mut curr = trie.as_ref();
        for (i, h) in hash.as_bytes().iter().enumerate() {
            match curr.get_direct_child(*h) {
                None => idx = i,
                Some(child) => {
                    curr = child;
                }
            }
        }

        if idx < TrieTracker::MIN_HASH_LEN {
            idx = TrieTracker::MIN_HASH_LEN;
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
    tokio::spawn(expire_timer(
        tracker.clone(),
        hash.to_owned().into(),
        dur.clone(),
        rx,
    ));

    trie.insert_value(hash.as_ref(), (hash.to_string(), to_save, tx));

    // Hash collision
    Ok(idx)
}

/// expire_timer waits on 2 futures:
/// 1. the timer
/// 2. the abort signal
/// If the timer finishes first, expire_timer removes the entry from `tracker.haystack`.
/// If the abort signal comes first, expire_timer simply returns `Ok(())`.
#[inline]
async fn expire_timer(
    tracker: Arc<TrieTracker>,
    hash: Vec<u8>,
    dur: Duration,
    abort: oneshot::Receiver<()>,
) -> Result<(), StoreError> {
    let hash_str = std::str::from_utf8(&hash).unwrap_or("hash is invalid utf-8");

    tokio::select! {
        // Set a timer to remove clipboard once it expires
        _ = tokio::time::sleep(dur) => {
             let mut trie = tracker.trie.lock().expect("failed to unlock trie");
                match trie.remove(&hash) {
                    Some(TrieNode{
                        value: Some((_, None, _sender)),
                        ..
                    }) => {
                        // clipboard is None if it's Clipboard::Persist
                        if let Err(err) = persist::rm_clipboard_file(hash_str) {
                            println!("error removing clipboard {hash_str} file: {}", err.to_string())
                        }
                    },

                    // Clipboard::Mem
                    Some(_) => {}

                    _ => {
                        println!(
                            "expire_timer: timer for {hash_str} ended, but there's no live clipboard",
                        )
                    },
               }
        }

        // If we get cancellation signal, return from this function
        _ = abort => {
            println!(
                "expire_timer: timer for {hash_str} extended for {dur:?}",
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn insert() {
        use super::*;
        let htrie = Arc::new(TrieTracker::new());
        let dur = Duration::from_millis(400);

        let clip = Clipboard::Mem("foo".into());
        assert_eq!(
            insert_clipboard(htrie.clone(), "____1", clip.clone(), dur)
                .expect("failed to insert"),
            4
        );
        assert_eq!(
            insert_clipboard(htrie.clone(), "____12", clip.clone(), dur)
                .expect("failed to insert"),
            5
        );
        assert_eq!(
            insert_clipboard(htrie.clone(), "____123", clip.clone(), dur)
                .expect("failed to insert"),
            6
        );
        assert_eq!(
            insert_clipboard(htrie.clone(), "____0", clip.clone(), dur)
                .expect("failed to insert"),
            4
        );
        assert_eq!(
            insert_clipboard(htrie.clone(), "____01", clip.clone(), dur)
                .expect("failed to insert"),
            5
        );
        assert_eq!(
            insert_clipboard(htrie.clone(), "____012", clip.clone(), dur)
                .expect("failed to insert"),
            6
        );

        // This insertion will last longer than the rest
        assert_eq!(
            insert_clipboard(
                htrie.clone(),
                "lingers",
                clip.clone(),
                Duration::from_millis(1500)
            )
            .expect("failed to insert"),
            6
        );

        // Wait for clipboards to expire
        tokio::spawn(tokio::time::sleep(dur)).await.unwrap();

        // Assert that they were all removed except for "lingers"
        assert_eq!(htrie.trie.lock().unwrap().all_children_values().len(), 1);
    }

    #[tokio::test]
    async fn e2e() {
        use super::*;

        let htrie = Arc::new(TrieTracker::new());
        let hash1 = "111_111111"; // min_frag = 4
        let hash2 = "111_222222"; // min_frag = 5

        let clip = Clipboard::Mem("foo".into());
        let dur = Duration::from_secs(5);

        assert_eq!(
            insert_clipboard(htrie.clone(), hash1, clip.clone(), dur).unwrap(),
            4
        );
        assert_eq!(
            insert_clipboard(htrie.clone(), hash2, clip.clone(), dur).unwrap(),
            5
        );
    }
}
