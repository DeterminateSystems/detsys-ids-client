use serde::{Deserialize, Serialize};

use crate::{Map, collator::FeatureFacts};

use super::{CoherentFeatureFlags, ServerOptions};

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Checkin {
    #[serde(default, skip_serializing)]
    pub(crate) server_options: ServerOptions,
    pub(crate) options: CoherentFeatureFlags,
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
