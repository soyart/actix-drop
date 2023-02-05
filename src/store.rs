use serde::Deserialize;

use crate::data::Data;

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

impl std::ops::Deref for Store {
    type Target = [u8];

    fn deref(self: &Self) -> &Self::Target {
        match self {
            Self::Mem(t) => return t.as_ref(),
            Self::Persist(t) => return t.as_ref(),
        }
    }
}

impl AsRef<[u8]> for Store {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Mem(t) => return t.as_ref(),
            Self::Persist(t) => return t.as_ref(),
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
    use super::{Data, Store};

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
