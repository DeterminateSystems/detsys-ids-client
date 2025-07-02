use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;

use crate::{Map, collator::FeatureFacts};

pub(crate) type CoherentFeatureFlags = HashMap<String, Arc<Feature<serde_json::Value>>>;

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Checkin {
    #[serde(default)]
    pub(crate) server_options: ServerOptions,
    pub(crate) options: CoherentFeatureFlags,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub(crate) struct ServerOptions {
    pub(crate) compression_algorithms: crate::compression_set::CompressionSet,
}

impl Checkin {
    pub(crate) fn as_feature_facts(&self) -> FeatureFacts {
        let mut feature_facts = Map::new();
        feature_facts.insert(
            "$active_feature_flags".into(),
            self.options
                .keys()
                .map(|v| serde_json::Value::from(v.to_owned()))
                .collect::<Vec<serde_json::Value>>()
                .into(),
        );

        for (name, feat) in self.options.iter() {
            feature_facts.insert(format!("$feature/{name}"), feat.variant.clone());
        }

        FeatureFacts(feature_facts)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Feature<T: serde::de::DeserializeOwned> {
    pub variant: serde_json::Value,
    #[serde(
        with = "crate::json_string",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub payload: Option<T>,
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse() {
        let json = r#"
        {

            "options": {
                "dni-det-msg-ptr": {
                    "variant": "a",
                    "payload": "\"dni-det-msg-a\""
                },
                "dni-det-msg-a": {
                    "variant": "a",
                    "payload": "\"hello\""
                },
                "fine-grained-tokens": {
                    "variant": false
                }
            }
        }"#;

        let _: super::Checkin = serde_json::from_str(json).unwrap();
    }
}
