use serde::{
    de::{self, Error, Visitor},
    Deserialize, Serialize,
};
use std::collections::BTreeMap;

#[derive(Eq, PartialEq, Clone, Serialize, Debug)]
pub struct CodeSnippet(pub BTreeMap<u32, String>);

impl CodeSnippet {
    pub fn new() -> Self { Self { 0: BTreeMap::new() } }
}

impl Default for CodeSnippet {
    fn default() -> Self { Self::new() }
}

struct CodeSnippetVisitor;

impl<'de> Visitor<'de> for CodeSnippetVisitor {
    type Value = CodeSnippet;

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut bm = BTreeMap::new();

        while let Ok(Some((key_str, value))) = map.next_entry::<String, String>() {
            let key = key_str.parse::<u32>();

            if let Err(e) = key {
                return Err(A::Error::custom(format!(
                    "String '{}' could not be parsed as u32: {}",
                    key_str, e
                )));
            }

            let key = key.unwrap();
            bm.insert(key, value);
        }

        Ok(CodeSnippet(bm))
    }

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(
            "a map with that adheres to <String, String> where the String key can be \
             parsed into a u32.",
        )
    }
}

impl<'de> Deserialize<'de> for CodeSnippet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(CodeSnippetVisitor)
    }
}
