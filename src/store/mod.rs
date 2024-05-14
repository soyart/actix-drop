pub mod clipboard;
pub mod data;
pub mod error;
pub mod persist;

use tokio::sync::oneshot;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clipboard::Clipboard;
use error::StoreError;

enum Storage {
    Memory(Clipboard),
    Persistent,
}

struct Entry {
    storage: Storage,
    abort_tx: oneshot::Sender<()>,
}

impl Entry {
    fn is_persisted(&self) -> bool {
        matches!(self.storage, Storage::Persistent)
    }
}

/// Store is used to store in-memory actix-drop clipboard
pub struct Store {
    /// If a clipboard is `Clipboard::Mem`, its hash gets inserted as map key with value `Some(_)`
    /// If a clipboard is `Clipboard::Persist`, its hash gets inserted as map key with value `None`
    /// The one-shot sender is for aborting the timeout timer
    haystack: Mutex<HashMap<String, Entry>>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            haystack: Mutex::new(HashMap::new()),
        }
    }

    /// store_new_clipboard stores new clipboard in Store.
    /// With each clipboard, a timer task will be dispatched
    /// to the background to expire it (see `async fn expire_timer`).
    /// If a new clipboard comes in with identical 4-byte hash,
    /// the previous clipboard timer thread is forced to return,
    /// and a the new clipboard with its own timer takes its place.
    pub fn store_new_clipboard(
        store: Arc<Self>,
        hash: &str,
        clipboard: Clipboard,
        dur: Duration,
    ) -> Result<(), StoreError> {
        // Drop the old timer for the hash key
        if let Some(entry) = store.remove_entry(&hash) {
            // Recevier might have been dropped
            if let Err(_) = entry.abort_tx.send(()) {
                eprintln!("store_new_clipboard: failed to remove old timer for {hash}");
            }
        }

        let to_save = match clipboard.clone() {
            // Clipboard::Mem(data) => data will have to live in haystack
            clip @ Clipboard::Mem(_) => Storage::Memory(clip),

            // Clipboard::Persist(data) => data does not have to live in haystack
            Clipboard::Persist(data) => {
                persist::write_clipboard_file(hash, data.as_ref())?;
                Storage::Persistent
            }
        };

        // Store will remember tx_abort to abort the timer in expire_timer.
        let (tx_abort, rx_abort) = oneshot::channel();
        tokio::task::spawn(cleanup(
            store.clone(),
            hash.to_owned(),
            dur.clone(),
            rx_abort,
        ));

        store
            .haystack
            .lock()
            .expect("failed to lock haystack")
            .insert(
                hash.to_owned(),
                Entry {
                    storage: to_save,
                    abort_tx: tx_abort,
                },
            );

        Ok(())
    }

    /// get_clipboard gets a clipboard whose entry key matches `hash`.
    /// Calling get_clipboard does not move the value out of haystack
    pub fn get_clipboard(&self, hash: &str) -> Option<Clipboard> {
        let mut haystack = self.haystack.lock().expect("failed to lock haystack");

        match haystack.get(hash) {
            None => None,

            Some(entry) => match &entry.storage {
                Storage::Persistent => match persist::read_clipboard_file(hash) {
                    Err(err) => {
                        eprintln!("error reading file {hash}: {}", err.to_string());

                        // Clear dangling persisted clipboard from haystack
                        haystack.remove(hash);
                        None
                    }

                    Ok(data) => Some(Clipboard::Persist(data.into())),
                },

                Storage::Memory(clipboard) => Some(clipboard.to_owned()),
            },
        }
    }

    fn remove_entry(&self, hash: &str) -> Option<Entry> {
        self.haystack
            .lock()
            .expect("failed to lock haystack")
            .remove(&hash.to_owned())
    }
}

/// Spawns async task with timer to remove clipboard once it expires.
///
/// cleanup waits on 2 futures:
/// 1. the timer
/// 2. the abort signal
/// If the timer finishes first, expire_timer removes the entry from `Store.haystack`.
/// If the abort signal comes first, expire_timer simply returns `Ok(())`.
async fn cleanup(
    store: Arc<Store>,
    hash: String,
    dur: Duration,
    abort: oneshot::Receiver<()>,
) -> Result<(), StoreError> {
    tokio::select! {
        // Set a timer to remove clipboard once it expires
        _ = tokio::time::sleep(dur) => {
            if let Some((_, entry)) = store.haystack
                    .lock()
                    .expect("failed to lock haystack")
                    .remove_entry(&hash)
            {
                if entry.is_persisted() {
                    persist::rm_clipboard_file(hash)?;
                }
            }
        }

        // If we get cancellation signal, return from this function
        _ = abort => {
            println!("expire_timer: timer for {hash} extended for {dur:?}");
        }
    }

    Ok(())
}

impl From<(Storage, oneshot::Sender<()>)> for Entry {
    fn from(value: (Storage, oneshot::Sender<()>)) -> Self {
        Self {
            storage: value.0,
            abort_tx: value.1,
        }
    }
}

impl From<Clipboard> for Storage {
    fn from(clip: Clipboard) -> Self {
        match clip {
            clip @ Clipboard::Mem(_) => Self::Memory(clip),
            Clipboard::Persist(_) => Self::Persistent,
        }
    }
}

#[cfg(test)]
#[allow(dead_code)] // Bad tests - actix/tokio runtime conflict, will come back later
mod tests {
    use super::*;

    #[test]
    fn test_store_get() {
        // We should be able to get multiple times
        let foo = "foo";
        let clip = Clipboard::Mem("eiei".into());
        let (tx, _) = oneshot::channel();
        let entry = Entry {
            storage: clip.into(),
            abort_tx: tx,
        };

        let store = Store::new();
        store
            .haystack
            .lock()
            .expect("failed to lock haystack")
            .insert(foo.to_owned(), entry);

        assert!(store.get_clipboard(foo).is_some());
        assert!(store.get_clipboard(foo).is_some());
        assert!(store.get_clipboard(foo).is_some());
    }

    #[tokio::test]
    async fn test_store_expire() {
        let store = Arc::new(Store::new());
        let key = "keyfoo";
        let dur = Duration::from_millis(300);

        // Store and launch the expire timer
        Store::store_new_clipboard(store.clone(), key, Clipboard::Mem("foo".into()), dur).unwrap();
        // Sleep until expired
        tokio::spawn(tokio::time::sleep(dur)).await.unwrap();

        // Clipboard with `key` should have been expired
        assert!(store.get_clipboard(key).is_none());
    }

    #[tokio::test]
    async fn test_reset_timer() {
        let hash = "keyfoo";
        let store = Arc::new(Store::new());

        let clipboard = Clipboard::Mem(vec![1u8, 2, 3].into());
        let dur200 = Duration::from_millis(200);
        let dur400 = Duration::from_millis(400);

        Store::store_new_clipboard(store.clone(), hash, clipboard.clone(), dur400)
            .expect("failed to store to Store");

        tokio::spawn(tokio::time::sleep(dur200)).await.unwrap();

        Store::store_new_clipboard(store.clone(), hash, clipboard, dur400)
            .expect("failed to re-write to Store");

        tokio::spawn(tokio::time::sleep(dur200)).await.unwrap();

        assert!(store.get_clipboard(hash).is_some());

        tokio::spawn(tokio::time::sleep(dur200)).await.unwrap();

        assert!(store.get_clipboard(hash).is_none());
    }
}
