use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;

use super::clipboard::Clipboard;
use super::error::StoreError;
use super::persist;

/// Tracker is used to store in-memory actix-drop clipboard
pub struct Tracker {
    // In-memory storage
    haystack: Mutex<HashMap<String, Option<Clipboard>>>,
    // The sender is used to send cancel message for the timer
    #[allow(dead_code)]
    stoppers: Mutex<HashMap<String, mpsc::Sender<()>>>,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            haystack: Mutex::new(HashMap::new()),
            stoppers: Mutex::new(HashMap::new()),
        }
    }

    pub fn store_new_clipboard(
        tracker: Arc<Self>,
        hash: &str,
        clipboard: Clipboard,
        dur: Duration,
    ) -> Result<(), StoreError> {
        // Save the clipboard and then add an entry to tracker
        clipboard.save_clipboard(hash)?;

        let to_save = match clipboard.clone() {
            clip @ Clipboard::Mem(_) => Some(clip),
            Clipboard::Persist(_) => None,
        };

        actix_web::rt::spawn(countdown_remove(
            tracker.clone(),
            hash.to_owned(),
            Duration::from_secs(dur.as_secs()),
        ));

        let mut handle = tracker.haystack.lock().unwrap();
        handle.insert(hash.to_string(), to_save);

        Ok(())
    }

    pub fn get_clipboard(&self, hash: &str) -> Option<Clipboard> {
        let handle = self.haystack.lock().unwrap();
        let entry = handle.get(&hash.to_string());

        if let Some(&Some(ref clipboard)) = entry {
            // Some(Some) = Clipboard::Mem
            // Return the clipboard in the Tracker
            return Some(clipboard.to_owned());
        } else if entry.is_some() {
            // Some(None) = Clipboard::Persist
            // Create and return new clipboard constructed from data in the file
            let mut clipboard = Clipboard::Persist(vec![].into());
            if let Err(err) = clipboard.read_clipboard(hash) {
                eprintln!("error reading file {}: {}", err.to_string(), hash);
                return None;
            }

            return Some(clipboard);
        }

        // None(None) = neither in file or Tracker
        None
    }
}

pub async fn countdown_remove(
    tracker: Arc<Tracker>,
    hash: String,
    dur: Duration,
) -> Result<(), StoreError> {
    actix_web::rt::time::sleep(dur).await;

    let mut handle = tracker.haystack.lock().unwrap();
    if let Some((_key, clipboard)) = handle.remove_entry(&hash.to_string()) {
        // Some(_, None) => clipboard persisted to disk
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

        let rt = actix_web::rt::Runtime::new().unwrap();
        let f1 = countdown_remove(tracker.clone(), foo.to_string(), dur);
        let f2 = countdown_remove(tracker.clone(), bar.to_string(), dur);

        rt.block_on(rt.spawn(f1))
            .unwrap()
            .expect("fail to spawn f1");
        rt.block_on(rt.spawn(f2))
            .unwrap()
            .expect("fail to spawn f2");

        if !tracker.haystack.lock().unwrap().is_empty() {
            panic!("tracker not empty after cleared");
        }
    }
}

// impl std::ops::Deref for Tracker {
//     type Target = Mutex<HashMap<String, Option<Clipboard>>>;
//     fn deref(&self) -> &Self::Target {
//         &self.haystack
//     }
// }
//
// impl std::ops::DerefMut for Tracker {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.haystack
//     }
// }
