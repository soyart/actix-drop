use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::clipboard::Clipboard;
use super::error::StoreError;
use super::persist;

/// Tracker is used to store in-memory actix-drop clipboard
pub struct Tracker(Mutex<HashMap<Arc<String>, Option<Clipboard>>>);

impl Tracker {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    pub fn store_new_clipboard(&self, hash: &str, clipboard: Clipboard) -> Result<(), StoreError> {
        // Save the clipboard and then add an entry to tracker
        clipboard.save_clipboard(hash)?;

        let to_save = match clipboard.clone() {
            clip @ Clipboard::Mem(_) => Some(clip),
            Clipboard::Persist(_) => None,
        };

        let mut handle = self.lock().unwrap();
        handle.insert(Arc::new(hash.to_string()), to_save);

        Ok(())
    }
}

pub async fn countdown_remove(
    tracker: Arc<Tracker>,
    hash: String,
    dur: Duration,
) -> Result<(), StoreError> {
    actix_web::rt::time::sleep(dur).await;

    let mut handle = tracker.lock().unwrap();
    if let Some((_key, clipboard)) = handle.remove_entry(&hash.to_string()) {
        // None = persisted to disk
        if clipboard.is_none() {
            persist::rm_clipboard_file(hash)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tracker_tests {
    use super::*;
    #[test]
    fn test_store_tracker() {
        let hash = "foo".to_string();
        let tracker = Tracker::new();
        if let Err(err) =
            tracker.store_new_clipboard(&hash, Clipboard::Persist("eiei".as_bytes().into()))
        {
            panic!("failed to insert: {}", err.to_string());
        }

        let dur = Duration::from_secs(1);
        let shared_tracker = Arc::new(tracker);
        let fut = countdown_remove(shared_tracker.clone(), hash, dur);
        let rt = actix_web::rt::Runtime::new().unwrap();
        rt.block_on(fut).unwrap();

        if !shared_tracker.to_owned().lock().unwrap().is_empty() {
            panic!("tracker not empty after cleared");
        }
    }
}

impl std::ops::Deref for Tracker {
    type Target = Mutex<HashMap<Arc<String>, Option<Clipboard>>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Tracker {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
