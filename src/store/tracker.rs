use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;

use super::clipboard::Clipboard;
use super::error::StoreError;
use super::persist;
use super::trie_tracker::TrieTracker;

/// Tracker is used to store in-memory actix-drop clipboard
pub struct Tracker {
    /// If a clipboard is `Clipboard::Mem`, its hash gets inserted as map key with value `Some(_)`
    /// If a clipboard is `Clipboard::Persist`, its hash gets inserted as map key with value `None`
    /// The one-shot sender is for aborting the timeout timer
    haystack: TrieTracker,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            haystack: TrieTracker::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.haystack.is_empty()
    }

    /// store_new_clipboard stores new clipboard in tracker.
    /// With each clipboard, a timer task will be dispatched
    /// to the background to expire it (see `async fn expire_timer`).
    /// If a new clipboard comes in with identical 4-byte hash,
    /// the previous clipboard timer thread is forced to return,
    /// and a the new clipboard with its own timer takes its place.
    pub fn store_new_clipboard(
        tracker: Arc<Self>,
        hash: &str,
        clipboard: Clipboard,
        dur: Duration,
    ) -> Result<usize, StoreError> {
        // Drop the old timer thread
        if let Some((_, stopper)) = tracker.remove(&hash) {
            // Recevier might have been dropped
            if let Err(_) = stopper.send(()) {
                eprintln!("store_new_clipboard: failed to remove old timer for {hash}");
            }
        }

        let (min_len, rx) = tracker.haystack.insert_clipboard(hash, clipboard)?;

        println!("spawing timer for {hash}");
        tokio::task::spawn(expire_timer(
            tracker.clone(),
            hash.to_owned(),
            dur.clone(),
            rx,
        ));

        Ok(min_len)
    }

    /// get_clipboard gets a clipboard whose entry key matches `hash`.
    /// Calling get_clipboard does not move the value out of haystack
    pub fn get_clipboard(&self, hash: &str) -> Option<Clipboard> {
        self.haystack.get_clipboard_frag(hash)
    }

    pub fn remove(&self, hash: &str) -> Option<(Option<Clipboard>, oneshot::Sender<()>)> {
        self.haystack
            .remove(hash)
            .and_then(|tuple| Some((tuple.1, tuple.2)))
    }
}

/// expire_timer waits on 2 futures:
/// 1. the timer
/// 2. the abort signal
/// If the timer finishes first, expire_timer removes the entry from `tracker.haystack`.
/// If the abort signal comes first, expire_timer simply returns `Ok(())`.
#[inline]
async fn expire_timer(
    tracker: Arc<Tracker>,
    hash: String,
    dur: Duration,
    abort: oneshot::Receiver<()>,
) -> Result<(), StoreError> {
    tokio::select! {
        // Expire the clipboard after dur.
        _ = tokio::time::sleep(dur) => {
                match tracker.haystack.remove(&hash) {
                    Some((_, clipboard, _)) => {
                        if clipboard.is_none() {
                            if let Err(err) = persist::rm_clipboard_file(&hash) {
                                eprintln!("failed to remove persisted clipboard {hash}");
                                return Err(err)
                            }
                        }

                        println!("expiring clipboard {hash}");
                    },

                    None => {
                        println!("no live clipboard {hash} to expire");
                    }
               }
        }

        // If we get cancellation signal, return from this function
        _ = abort => {
            println!(
                "expire_timer: timer for {hash} extended for {dur:?}",
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_get() {
        let t = Arc::new(Tracker::new());
        let key = "keyfoo";
        let bad_key = "badkey";
        let dur = Duration::from_millis(300);

        Tracker::store_new_clipboard(t.clone(), key, Clipboard::Mem("foo".into()), dur)
            .unwrap();

        assert!(t.get_clipboard(key).is_some());
        assert!(t.get_clipboard(bad_key).is_none());
    }

    #[tokio::test]
    async fn test_store_expire() {
        let t = Arc::new(Tracker::new());
        let key = "keyfoo";
        let dur = Duration::from_millis(300);

        // Store and launch the expire timer
        Tracker::store_new_clipboard(t.clone(), key, Clipboard::Mem("foo".into()), dur)
            .unwrap();
        // Sleep until expired
        tokio::spawn(tokio::time::sleep(dur)).await.unwrap();

        // Clipboard with `key` should have been expired
        assert!(t.get_clipboard(key).is_none());
    }

    #[tokio::test]
    async fn test_reset_timer() {
        let hash = "keyfoo";
        let tracker = Arc::new(Tracker::new());

        let clipboard = Clipboard::Mem(vec![1u8, 2, 3].into());
        let dur200 = Duration::from_millis(200);
        let dur400 = Duration::from_millis(400);

        Tracker::store_new_clipboard(tracker.clone(), hash, clipboard.clone(), dur400)
            .expect("failed to store to tracker");

        tokio::spawn(tokio::time::sleep(dur200)).await.unwrap();

        Tracker::store_new_clipboard(tracker.clone(), hash, clipboard, dur400)
            .expect("failed to re-write to tracker");

        tokio::spawn(tokio::time::sleep(dur200)).await.unwrap();

        assert!(tracker.get_clipboard(hash).is_some());

        tokio::spawn(tokio::time::sleep(dur200)).await.unwrap();

        assert!(tracker.get_clipboard(hash).is_none());
    }

    #[tokio::test]
    async fn e2e() {
        let tracker = Arc::new(Tracker::new());

        let vals = vec![
            // (path, expected return values from insert_clipboard/store_clipboard, final minimum key length)
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
            Tracker::store_new_clipboard(
                tracker.clone(),
                val.0,
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
                assert!(tracker.get_clipboard(&vals[i].0[..=vals[i].2]).is_some());
            });

        // Wait for clipboards to expire
        tokio::spawn(tokio::time::sleep(Duration::from_millis(600)))
            .await
            .unwrap();

        // All clipboards should have expired
        assert!(tracker.is_empty());

        // Try insert, then re-insert with longer duration,
        // then sleep for short duration.
        Tracker::store_new_clipboard(
            tracker.clone(),
            "some_long_ass_key",
            Clipboard::Mem("foo".into()),
            Duration::from_millis(500),
        )
        .expect("failed to store new foo clipboard");

        tokio::spawn(tokio::time::sleep(Duration::from_millis(500)))
            .await
            .unwrap();

        Tracker::store_new_clipboard(
            tracker.clone(),
            "some_long_ass_key",
            Clipboard::Mem("foo".into()),
            Duration::from_secs(2),
        )
        .expect("failed to store new foo clipboard");

        tokio::spawn(tokio::time::sleep(Duration::from_millis(200)))
            .await
            .unwrap();

        // The clipboard foo must still live.
        assert!(tracker.get_clipboard("some_long").is_some());
    }
}
