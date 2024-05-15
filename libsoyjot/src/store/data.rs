use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer,
};

/// Data represents clipboard data as bytes.
/// Valid strings (&str and String) can be deserialized into Data.
#[derive(Clone, Deserialize)]
pub struct Data(#[serde(deserialize_with = "string_or_bytes")] pub Vec<u8>);

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<'a, T> From<T> for Data
where
    T: Into<Vec<u8>>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl TryInto<String> for Data {
    type Error = std::string::FromUtf8Error;
    fn try_into(self) -> Result<String, Self::Error> {
        String::from_utf8(self.0)
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
