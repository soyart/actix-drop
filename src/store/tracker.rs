use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::clipboard::Clipboard;
use super::error::StoreError;
use super::persist;

/// Tracker is used to store in-memory actix-drop clipboard
pub struct Tracker {
    // In-memory storage
    haystack: Mutex<HashMap<String, Option<Clipboard>>>,
    // The sender is used to send cancel message for the timer
    stoppers: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            haystack: Mutex::new(HashMap::new()),
            stoppers: Mutex::new(HashMap::new()),
        }
    }

    // store_new_clipboard stores new clipboard in tracker. With each clipboard, a thread with timer will be launched
    // in the background to expire it. If a new clipboard comes and it 4-byte hash matches the
    // existing one, the previous clipboard timer thread is forced to return, and a new timer for
    // the new clipboard takes its place.
    pub fn store_new_clipboard(
        tracker: Arc<Self>,
        hash: &str,
        clipboard: Clipboard,
        dur: Duration,
    ) -> Result<(), StoreError> {
        // Drop the old timer thread
        if let Some(stopper) = tracker.get_stopper(&hash) {
            // Recevier might have been dropped
            if let Err(_) = stopper.send(()) {
                eprintln!("store_new_clipboard: failed to remove old timer for {hash}");
            }
        }

        // Save the clipboard and then add an entry to tracker
        clipboard.save_clipboard(hash)?;

        let to_save = match clipboard.clone() {
            clip @ Clipboard::Mem(_) => Some(clip),
            Clipboard::Persist(_) => None,
        };

        tracker
            .haystack
            .lock()
            .expect("failed to lock haystack")
            .insert(hash.to_owned(), to_save);

        let (tx, rx) = oneshot::channel();

        tracker
            .stoppers
            .lock()
            .expect("failed to lock stoppers")
            .insert(hash.to_owned(), tx);

        tokio::task::spawn(expire_timer(
            tracker.clone(),
            rx,
            hash.to_owned(),
            Duration::from_secs(dur.as_secs()),
        ));

        Ok(())
    }

    pub fn get_clipboard(&self, hash: &str) -> Option<Clipboard> {
        let mut handle = self.haystack.lock().expect("failed to lock haystack");
        let entry = handle.get(hash);

        match entry {
            // Clipboard::Mem
            Some(&Some(ref clipboard)) => {
                return Some(clipboard.to_owned());
            }

            // Clipboard::Persist
            Some(None) => {
                let mut clipboard = Clipboard::Persist(vec![].into());
                // If we could not read the file, remove it from haystack
                if let Err(err) = clipboard.read_clipboard(hash) {
                    eprintln!("error reading file {}: {}", err.to_string(), hash);

                    handle.remove(hash);
                    return None;
                }

                Some(clipboard)
            }

            None => None,
        }
    }

    // Get stopper removed the Sender from self.stoppers, and the caller can use the value
    // to send cancellaton signal to the closure waiting on corresponding timer.
    pub fn get_stopper(&self, hash: &str) -> Option<oneshot::Sender<()>> {
        self.stoppers
            .lock()
            .expect("failed to lock stoppers")
            .remove(&hash.to_owned())
    }
}

pub async fn expire_timer(
    tracker: Arc<Tracker>,
    abort: oneshot::Receiver<()>,
    hash: String,
    dur: Duration,
) -> Result<(), StoreError> {
    tokio::select! {
        // Set a timer to remove clipboard once it expires
        _ = tokio::time::sleep(dur) => {
        if let Some((_key, clipboard)) = tracker.haystack
                .lock()
                .expect("failed to lock haystack")
                .remove_entry(&hash)
        {
            // Some(_, None) => clipboard persisted to disk
            if clipboard.is_none() {
                persist::rm_clipboard_file(hash)?;
            }
        }

    }
        // If we get cancellation signal, return from this function
        _ = abort => {
            println!("countdown_remove: timer for {hash} extended for {dur:?}");
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(dead_code)] // Bad tests - actix/tokio runtime conflict, will come back later
mod tracker_tests {
    use super::*;
    #[test]
    fn test_store_tracker() {
        let foo = "foo";
        let bar = "bar";
        let hashes = vec![foo, bar];

        let tracker = Arc::new(Tracker::new());
        let dur = Duration::from_secs(1);
        for hash in hashes {
            Tracker::store_new_clipboard(
                tracker.clone(),
                &hash,
                Clipboard::Persist("eiei".as_bytes().into()),
                dur,
            )
            .expect("failed to insert into tracker");
        }

        let (_, r1) = oneshot::channel();
        let (_, r2) = oneshot::channel();
        let f1 = expire_timer(tracker.clone(), r1, foo.to_string(), dur);
        let f2 = expire_timer(tracker.clone(), r2, bar.to_string(), dur);

        let rt = actix_web::rt::Runtime::new().unwrap();

        rt.block_on(actix_web::rt::spawn(f1))
            .unwrap()
            .expect("fail to spawn f1");
        rt.block_on(actix_web::rt::spawn(f2))
            .unwrap()
            .expect("fail to spawn f2");

        if !tracker.haystack.lock().unwrap().is_empty() {
            panic!("tracker not empty after cleared");
        }
    }

    #[test]
    fn test_reset_timer() {
        let hash = "foo";
        let tracker = Arc::new(Tracker::new());

        let clipboard = Clipboard::Mem(vec![1u8, 2, 3].into());
        let two_secs = Duration::from_secs(2);
        let four_secs = Duration::from_secs(4);

        Tracker::store_new_clipboard(tracker.clone(), hash, clipboard.clone(), four_secs)
            .expect("failed to store to tracker");

        let rt = actix_web::rt::Runtime::new().unwrap();

        rt.block_on(rt.spawn(actix_web::rt::time::sleep(two_secs)))
            .expect("failed to sleep-block");

        Tracker::store_new_clipboard(tracker.clone(), hash, clipboard, four_secs)
            .expect("failed to re-write to tracker");

        rt.block_on(rt.spawn(actix_web::rt::time::sleep(two_secs)))
            .expect("failed to sleep-block");

        assert!(tracker.get_clipboard(hash).is_some());
    }
}
