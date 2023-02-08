use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Tracker(HashMap<String, (super::Store, Instant)>);

impl Tracker {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl std::ops::Deref for Tracker {
    type Target = HashMap<String, (super::Store, Instant)>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Tracker {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub fn loop_add_tracker(
    recv: mpsc::Receiver<(String, super::Store)>,
    tracker: Arc<Mutex<Tracker>>,
) {
    for new_clipboard in recv {
        println!("found new clipboard {}", new_clipboard.0);
        tracker
            .lock()
            .unwrap()
            .insert(new_clipboard.0, (new_clipboard.1, Instant::now()));
    }
}

pub fn clear_expired_clipboards(tracker: Arc<Mutex<Tracker>>, dur: Duration) {
    let mut expireds: Vec<String> = vec![];

    loop {
        let mut tracker = tracker.lock().unwrap();
        for expired in &expireds {
            tracker.remove(expired);
        }

        expireds = vec![];

        // TODO: Fix this fixed sleep
        std::thread::sleep(dur);

        for (hash_key, (store, timeout)) in tracker.iter() {
            if timeout.elapsed() > dur {
                match store {
                    super::Store::Mem(_) => {}
                    super::Store::Persist(_) => {
                        if let Err(err) = super::persist::rm_clipboard_file(hash_key) {
                            panic!(
                                "failed to remove expired clipboard file: {}",
                                err.to_string()
                            );
                        }

                        expireds.push(hash_key.clone());
                    }
                }
            }
        }
    }
}
