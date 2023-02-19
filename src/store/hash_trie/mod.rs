#![allow(dead_code)]

pub mod trie;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::clipboard::Clipboard;
use super::error::StoreError;
use super::persist;
use trie::{Trie, TrieNode};

pub struct TrieTracker {
    /// trie stores hash as a trie tree. Full SHA2 hash is inserted
    /// to the trie, and users will need to have at least N bytes of
    /// hash prefix (stored in key_lengths) in order to access the values.
    trie: Mutex<Trie<u8, (Option<Clipboard>, oneshot::Sender<()>)>>,
}

impl TrieTracker {
    const MIN_HASH_LEN: usize = 4;

    fn new() -> Self {
        Self {
            trie: Trie::new().into(),
        }
    }
}

fn insert(
    tracker: Arc<TrieTracker>,
    hash: &str,
    clipboard: Clipboard,
    dur: Duration,
    // The usize returned is the shortest hash length for which the hash
    // can still be uniquely accessed.
) -> Result<usize, StoreError> {
    let mut trie = tracker.trie.lock().unwrap();
    let mut idx = 0;

    // Find the idx return value
    {
        let mut curr = trie.as_ref();

        for (i, h) in hash.as_bytes().iter().enumerate() {
            match curr.search_direct_child(*h) {
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

    // Abort previous timer, and delete persisted file if there's one
    trie.root
        .remove(hash.as_ref())
        .and_then(|target_child| target_child.value)
        .map(|value| {
            // Clipboard::Mem has None stored
            if value.0.is_none() {
                persist::rm_clipboard_file(hash)?;
            }

            Ok::<tokio::sync::oneshot::Sender<()>, StoreError>(value.1)
        })
        .transpose()?
        .map(|aborter| match aborter.send(()) {
            Err(_) => Err(StoreError::Bug(format!(
                "failed to abort prev timer: {hash}",
            ))),

            _ => Ok(()),
        })
        .transpose()?;

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
    tokio::task::spawn(expire_timer(
        tracker.clone(),
        hash.to_owned().into(),
        dur.clone(),
        rx,
    ));

    trie.insert(hash.as_ref(), (to_save, tx));

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
             match tracker.trie.lock().expect("failed to unlock trie").remove(&hash) {
                Some(TrieNode{
                    value: Some((None, _sender)),
                    ..
                }) => {
                    // clipboard is None if it's Clipboard::Persist
                    if let Err(err) = persist::rm_clipboard_file(hash_str) {
                        println!("error removing clipboard {hash_str} file: {}", err.to_string())
                    }
                },

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
    #[test]
    fn test_wrapper() {
        use super::*;
        let htrie = Arc::new(TrieTracker::new());
        let dur = Duration::from_secs(1);

        let clip = Clipboard::Mem("foo".into());
        assert_eq!(
            insert(htrie.clone(), "____1", clip.clone(), dur).unwrap(),
            4
        );
        assert_eq!(
            insert(htrie.clone(), "____12", clip.clone(), dur).unwrap(),
            5
        );
        assert_eq!(
            insert(htrie.clone(), "____123", clip.clone(), dur).unwrap(),
            6
        );
        assert_eq!(
            insert(htrie.clone(), "____0", clip.clone(), dur).unwrap(),
            4
        );
        assert_eq!(
            insert(htrie.clone(), "____01", clip.clone(), dur).unwrap(),
            5
        );
        assert_eq!(insert(htrie, "____012", clip.clone(), dur).unwrap(), 6);
    }
}
