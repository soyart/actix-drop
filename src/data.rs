use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

pub const MEM: &str = "mem";
pub const PERSIST: &str = "persist";

#[derive(Deserialize)]
pub struct Data(#[serde(deserialize_with = "string_or_bytes")] Vec<u8>);

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn string_or_bytes<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrBytes(std::marker::PhantomData<fn() -> Vec<u8>>);

    impl<'de> Visitor<'de> for StringOrBytes {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("string or sequence")
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<Vec<u8>, E>
        where
            E: de::Error,
        {
            Ok(value.as_bytes().to_vec())
        }

        fn visit_seq<V>(self, mut visitor: V) -> std::result::Result<Vec<u8>, V::Error>
        where
            V: SeqAccess<'de>,
        {
            let mut vec = Vec::new();

            while let Some(element) = visitor.next_element()? {
                vec.push(element)
            }

            Ok(vec)
        }
    }

    deserializer.deserialize_any(StringOrBytes(std::marker::PhantomData))
}

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
    #[test]
    fn test_display_store() {
        use super::{Data, Store};
        let mem_str = Store::Mem(Data("foo".bytes().collect()));
        assert_eq!(r#""mem":"foo""#, format!("{:?}", mem_str));

        let persist_bin = Store::Persist(Data(vec![14, 16, 200]));
        assert_eq!(r#""persist":"[14, 16, 200]"#, format!("{:?}", persist_bin));

        // Valid UTF-8 byte array should be formatted as string
        let mem_str_vec = Store::Mem(Data("foo".bytes().collect::<Vec<u8>>()));
        assert_eq!(r#""mem":"foo""#, format!("{:?}", mem_str_vec));
    }
}
