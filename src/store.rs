use serde::Deserialize;

mod data;
pub mod error;
pub mod persist;

use data::Data;
use error::StoreError;

pub const MEM: &str = "mem";
pub const PERSIST: &str = "persist";

// enum Store specifies which type of storage to use
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "store", content = "data")]
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
            Self::Mem(_) => Err(error::StoreError::NotImplemented),
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

                Self::Mem(_) => Err(error::StoreError::NotImplemented),
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
        let key = match self {
            Self::Persist(_) => PERSIST,
            Self::Mem(_) => MEM,
        };

        let val = self.as_ref();
        let s = std::str::from_utf8(val);

        if let Ok(string) = s {
            write!(formatter, r#""{}":"{}""#, key, string)
        } else {
            write!(formatter, r#""{}":"{:?}"#, key, val)
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
