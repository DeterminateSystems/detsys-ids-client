use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Feature<T: serde::ser::Serialize + serde::de::DeserializeOwned> {
    pub variant: serde_json::Value,
    #[serde(
        with = "crate::json_string",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub payload: Option<T>,
}
