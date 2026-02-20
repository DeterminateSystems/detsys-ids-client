use std::{collections::HashMap, sync::Arc};

use super::Feature;

pub(crate) type CoherentFeatureFlags = HashMap<String, Arc<Feature<serde_json::Value>>>;
