//! Tagged enum buffers expose JSON object keys as strings, even for u32 map keys.
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use std::{collections::BTreeMap, fmt, path::PathBuf};
type Posts = BTreeMap<u32, BTreeMap<PathBuf, String>>;
#[derive(Deserialize)]
#[serde(untagged)]
enum Seq {
    Number(u32),
    Text(String),
}
pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Posts, D::Error> {
    struct PostsVisitor;
    impl<'de> Visitor<'de> for PostsVisitor {
        type Value = Posts;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a map of canonical u32 operation indices to post-state hashes")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Posts, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<Seq, BTreeMap<PathBuf, String>>()? {
                let index = match key {
                    Seq::Number(n) => n,
                    Seq::Text(text) => {
                        let n = text.parse::<u32>().map_err(de::Error::custom)?;
                        if n.to_string() != text {
                            return Err(de::Error::custom("non-canonical operation index"));
                        }
                        n
                    }
                };
                if result.insert(index, value).is_some() {
                    return Err(de::Error::custom("duplicate operation index"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(PostsVisitor)
}
