use serde::Deserialize;
// use std::collections::HashMap;

use super::data::Data;
use super::error::StoreError;

pub const MEM: &str = "mem";
pub const PERSIST: &str = "persist";

/// Store enumerates over types of storage to use for a clipboard,
/// with clipboard data as the value.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Clipboard {
    Mem(Data),
    Persist(Data),
}

impl Clipboard {
    #[allow(dead_code)]
    pub fn new(t: &str) -> Self {
        match t {
            PERSIST => Self::Persist(Vec::new().into()),
            _ => Self::Mem(Vec::new().into()),
        }
    }

    pub fn new_with_data<T>(t: &str, data: T) -> Self
    where
        T: Into<Data>,
    {
        match t {
            PERSIST => Self::Persist(data.into()),
            _ => Self::Mem(data.into()),
        }
    }

    pub fn is_implemented(&self) -> Result<(), StoreError> {
        Ok(())
    }

    pub fn key(&self) -> String {
        match self {
            Self::Mem(_) => MEM.to_string(),
            Self::Persist(_) => PERSIST.to_string(),
        }
    }
}

impl std::ops::Deref for Clipboard {
    type Target = [u8];

    fn deref(self: &Self) -> &Self::Target {
        match self {
            Self::Mem(data) => return data.as_ref(),
            Self::Persist(data) => return data.as_ref(),
        }
    }
}

impl AsRef<Data> for Clipboard {
    fn as_ref(&self) -> &Data {
        match self {
            Self::Mem(data) => data,
            Self::Persist(data) => data,
        }
    }
}

impl AsRef<[u8]> for Clipboard {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Mem(data) => return data.as_ref(),
            Self::Persist(data) => return data.as_ref(),
        }
    }
}

impl std::fmt::Debug for Clipboard {
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
    use super::{Clipboard, Data};

    #[test]
    fn test_store_debug() {
        let mem_str = Clipboard::Mem("foo".into());
        assert_eq!(r#""mem":"foo""#, format!("{:?}", mem_str));

        let persist_bin = Clipboard::Persist(Data(vec![14, 16, 200]));
        assert_eq!(r#""persist":"[14, 16, 200]"#, format!("{:?}", persist_bin));

        // Valid UTF-8 byte array should be formatted as string
        let mem_str_vec = Clipboard::Mem("bar".into());
        assert_eq!(r#""mem":"bar""#, format!("{:?}", mem_str_vec));
    }
}
