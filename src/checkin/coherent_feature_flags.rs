use std::{collections::BTreeMap, sync::Arc};

use super::Feature;

pub(crate) type CoherentFeatureFlags = BTreeMap<String, Arc<Feature<serde_json::Value>>>;
