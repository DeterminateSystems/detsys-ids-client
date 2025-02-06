// Lifted from https://github.com/serde-rs/serde/issues/994#issuecomment-316895860

use serde::de::{self, Deserialize, DeserializeOwned, Deserializer};

pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    let j = String::deserialize(deserializer)?;
    serde_json::from_str(&j).map_err(de::Error::custom)
}
