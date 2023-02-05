use serde::Deserialize;

mod data;
pub mod error;
pub mod persist;

use data::Data;
use error::StoreError;

pub const MEM: &str = "mem";
pub const PERSIST: &str = "persist";

/// Store enumerates over types of storage to use for a clipboard,
/// with clipboard data as the value.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "store", content = "data")] // TODO: remove this and use format { "mem": "data bytes" }
                                          // (HTML form forces us to use this style for now)
pub enum Store {
    Mem(Data),
    Persist(Data),
}

impl Store {
    pub fn new(t: &str) -> Self {
        match t {
            PERSIST => Self::Persist(Vec::new().into()),
            _ => Self::Mem(Vec::new().into()),
        }
    }

    pub fn key(&self) -> &str {
        match self {
            Self::Mem(_) => MEM,
            Self::Persist(_) => PERSIST,
        }
    }

    pub fn save_clipboard(&self, hash: &str) -> Result<(), error::StoreError> {
        match self {
            Self::Persist(data) => persist::write_clipboard_file(hash, data.as_ref()),
            Self::Mem(_) => Err(error::StoreError::NotImplemented(
                "write to mem".to_string(),
            )),
        }
    }

    // read_clipboard reads clipboard from source and saves the data to self.
    // If self is not empty, StoreError::NotImplemented is returned
    pub fn read_clipboard(&mut self, hash: &str) -> Result<(), error::StoreError> {
        if self.is_empty() {
            match self {
                Self::Persist(data) => {
                    data.0 = persist::read_clipboard_file(hash)?;
                    Ok(())
                }

                Self::Mem(_) => Err(error::StoreError::NotImplemented(
                    "read from mem".to_string(),
                )),
            }
        } else {
            Err(StoreError::Bug(
                "clipboard not empty before read".to_string(),
            ))
        }
    }
}

impl std::ops::Deref for Store {
    type Target = [u8];

    fn deref(self: &Self) -> &Self::Target {
        match self {
            Self::Mem(data) => return data.as_ref(),
            Self::Persist(data) => return data.as_ref(),
        }
    }
}

impl AsRef<Data> for Store {
    fn as_ref(&self) -> &Data {
        match self {
            Self::Mem(data) => data,
            Self::Persist(data) => data,
        }
    }
}

impl AsRef<[u8]> for Store {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Mem(data) => return data.as_ref(),
            Self::Persist(data) => return data.as_ref(),
        }
    }
}

impl std::fmt::Debug for Store {
    fn fmt(self: &Self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes: &[u8] = self.as_ref();

        if let Ok(string) = std::str::from_utf8(bytes) {
            write!(formatter, r#""{}":"{}""#, self.key(), string)
        } else {
            write!(formatter, r#""{}":"{:?}"#, self.key(), bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{data::Data, Store};

    #[test]
    fn test_store_debug() {
        let mem_str = Store::Mem("foo".into());
        assert_eq!(r#""mem":"foo""#, format!("{:?}", mem_str));

        let persist_bin = Store::Persist(Data(vec![14, 16, 200]));
        assert_eq!(r#""persist":"[14, 16, 200]"#, format!("{:?}", persist_bin));

        // Valid UTF-8 byte array should be formatted as string
        let mem_str_vec = Store::Mem("bar".into());
        assert_eq!(r#""mem":"bar""#, format!("{:?}", mem_str_vec));
    }
}
