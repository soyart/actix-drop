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
            Some(children) if children.len() != 1 => None,

            Some(child) => {
                match child[0].value {
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

    // Inserts clipboard to the tracker. If the clipboard is duplicate (i.e. the hash collides),
    // aborts the timer previously set and start a new one. It returns the minimum length of hash (frag)
    // required to uniquely access this clipboard. `insert_clipboard` expects that the hashes are of
    // uniform, constant length.
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
        tokio::spawn(expire_timer(
            tracker.clone(),
            hash.to_owned().into(),
            dur.clone(),
            rx,
        ));

        trie.insert_value(hash.as_ref(), (hash.to_string(), to_save, tx));

        Ok(min_len)
    }
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
        // Expire the clipboard after dur.
        _ = tokio::time::sleep(dur) => {
                match tracker.trie.lock().expect("failed to lock trie").remove(&hash) {
                    Some(TrieNode{
                        value: Some((_, None, _sender)),
                        ..
                    }) => {
                        // clipboard is None if it's Clipboard::Persist
                        if let Err(err) = persist::rm_clipboard_file(hash_str) {
                            println!("expire_timer: error removing clipboard {hash_str} file: {}", err.to_string())
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
    use super::*;

    // Test expiration works
    #[tokio::test]
    #[ignore]
    async fn expire() {
        let htrie = Arc::new(TrieTracker::new());

        // Expires in 2 secs
        TrieTracker::insert_clipboard(
            htrie.clone(),
            "some_long_ass_key",
            Clipboard::Mem("foo".into()),
            Duration::from_secs(2),
        )
        .unwrap();

        // Sleep for 1.2 sec
        tokio::spawn(tokio::time::sleep(Duration::from_millis(1200)))
            .await
            .unwrap();

        // It should still be there
        assert!(htrie.get_clipboard_frag("some_long_ass_key").is_some());

        // But if we sleep some more
        tokio::spawn(tokio::time::sleep(Duration::from_millis(800)))
            .await
            .unwrap();

        // It should have been gone
        assert!(htrie.get_clipboard_frag("some_long_ass_key").is_none());
    }

    // manual_test must not panic
    #[tokio::test]
    async fn manual_test() {
        let htrie = Arc::new(TrieTracker::new());

        let vals = vec![("123456", 4), ("1234567", 5), ("12345678", 6)];

        let dur = Duration::from_secs(3);
        vals.clone().into_iter().for_each(|val| {
            TrieTracker::insert_clipboard(
                htrie.clone(),
                val.0.as_ref(),
                Clipboard::Mem(val.0.into()),
                dur,
            )
            .expect("failed to insert");
        });

        let trie = htrie.trie.lock().unwrap();
        let mut _curr = trie.as_ref();
        _curr = _curr.get_child(b"1234").unwrap();
        _curr = _curr.get_child(b"5").unwrap();
        _curr = _curr.get_child(b"67").unwrap();
        _curr = _curr.get_child(b"8").unwrap();
    }

    #[tokio::test]
    async fn insert() {
        use super::*;
        let htrie = Arc::new(TrieTracker::new());
        let dur = Duration::from_secs(3);

        let vals = vec![
            ("123400000", 4), // Accessing this node requires designed minimum length 4
            ("123450000", 5), // Accessing this now requires 5 characters
            ("123456780", 6), // and so on..
            ("abcd1234x", 4),
            ("abcd12345", 9), // We need to go all the way "down" to character '5' to get unique value
            ("abcd00000", 5), // Here we only need 5 characters ("abcd0") to distinguish it from other nodes with "abcd"*
        ];

        vals.into_iter().enumerate().for_each(|(idx, val)| {
            println!("{} {v}", idx + 1, v = val.0);
            assert_eq!(
                TrieTracker::insert_clipboard(
                    htrie.clone(),
                    val.0.as_ref(),
                    Clipboard::Mem(val.0.into()),
                    dur,
                )
                .expect("failed to insert {val}"),
                // Expected return value
                val.1,
            );
        });
    }

    #[tokio::test]
    async fn e2e() {
        let htrie = Arc::new(TrieTracker::new());

        let vals = vec![
            // (path, expected return values from insert_clipboard, final minimum key length)
            // After all has been inserted, each path should be accessible with length equal to the
            // last tuple element
            ("123400000", 4, 5), // Accessing this node requires designed minimum length 4
            ("123450000", 5, 5), // Accessing this now requires 5 characters
            ("123456780", 6, 6), // and so on..
            ("abcd1234x00", 4, 9),
            ("abcd1234500", 9, 9), // We need to go all the way "down" to character '5' to get unique value
            ("abcd0000000", 5, 5), // Here we only need 5 characters ("abcd0") to distinguish it from other nodes with "abcd"*
        ];

        let dur = Duration::from_millis(500);
        vals.clone().into_iter().enumerate().for_each(|(_i, val)| {
            TrieTracker::insert_clipboard(
                htrie.clone(),
                val.0.as_ref(),
                Clipboard::Mem(val.0.into()),
                dur,
            )
            .unwrap();
        });

        vals.clone()
            .into_iter()
            .enumerate()
            .for_each(|(i, _min_len)| {
                println!("{} {v}", i + 1, v = vals[i].0);
                assert!(htrie.get_clipboard_frag(&vals[i].0[..=vals[i].2]).is_some());
            });

        // Wait for clipboards to expire
        tokio::spawn(tokio::time::sleep(Duration::from_millis(600)))
            .await
            .unwrap();

        // All clipboards should have expired
        assert!(htrie.trie.lock().unwrap().all_children_values().is_empty());

        // Try insert, then re-insert with longer duration,
        // then sleep for short duration.
        TrieTracker::insert_clipboard(
            htrie.clone(),
            "some_long_ass_key",
            Clipboard::Mem("foo".into()),
            Duration::from_millis(500),
        )
        .unwrap();

        tokio::spawn(tokio::time::sleep(Duration::from_millis(500)))
            .await
            .unwrap();

        TrieTracker::insert_clipboard(
            htrie.clone(),
            "some_long_ass_key",
            Clipboard::Mem("foo".into()),
            Duration::from_secs(2),
        )
        .unwrap();

        tokio::spawn(tokio::time::sleep(Duration::from_millis(200)))
            .await
            .unwrap();

        // The clipboard foo must still live.
        assert!(htrie.get_clipboard_frag("some_long").is_some());
    }
}
