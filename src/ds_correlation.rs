use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;

use serde::Deserialize;

use crate::{DeviceId, DistinctId, Map};

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum CorrelationInputs {
    DetSysTs(DetsysTsGitHubAction),
    Direct(Correlation),
}

type Groups = HashMap<String, Option<String>>;

impl Correlation {
    #[tracing::instrument]
    pub(crate) fn import() -> Correlation {
        let Some(correlation) = std::env::var_os("DETSYS_CORRELATION") else {
            return Correlation::default();
        };

        match serde_json::from_slice(correlation.as_bytes()) {
            Ok(CorrelationInputs::DetSysTs(a)) => a.into_correlation(),
            Ok(CorrelationInputs::Direct(a)) => a,
            Err(e) => {
                tracing::trace!(%e, "DETSYS_CORRELATION isn't parsable into a map");
                Correlation::default()
            }
        }
    }

    pub(crate) fn groups_as_map(&self) -> Map {
        self.groups
            .clone()
            .into_iter()
            .filter_map(|(k, v)| Some((k, v?.into())))
            .collect()
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub(crate) struct Correlation {
    pub(crate) distinct_id: Option<DistinctId>,

    #[serde(rename = "$anon_distinct_id")]
    pub(crate) anon_distinct_id: Option<String>,

    #[serde(rename = "$session_id")]
    pub(crate) session_id: Option<String>,

    #[serde(rename = "$window_id")]
    pub(crate) window_id: Option<String>,

    #[serde(rename = "$device_id")]
    pub(crate) device_id: Option<DeviceId>,

    #[serde(rename = "$groups", default)]
    pub(crate) groups: Groups,

    #[serde(flatten, default)]
    pub(crate) properties: Map,
}

#[derive(Deserialize, Debug, Clone)]
struct DetsysTsGitHubAction {
    // approximately the distinct_id / user identifier
    repository: Option<String>,

    // approximately $session_id
    run: Option<String>,

    // approximately $window_id
    run_differentiator: Option<String>,

    // approximately $device_id
    workflow: Option<String>,

    #[serde(default)]
    groups: Groups,

    #[serde(flatten, default)]
    extra_properties: Correlation,
}

impl DetsysTsGitHubAction {
    fn into_correlation(self) -> Correlation {
        let groups = self
            .groups
            .into_iter()
            .chain(self.extra_properties.groups)
            // We're going to merge two hashmaps, and only want to keep the ones with Some(value),
            // in case there are duplicates but one side has Some and the other has None.
            // So we filter out the Nones, then make them Options again.
            .filter_map(|(k, v)| Some((k, Some(v?))))
            .collect::<Groups>();

        Correlation {
            distinct_id: self
                .extra_properties
                .distinct_id
                .or_else(|| self.repository.clone().map(DistinctId::from)),
            anon_distinct_id: self.extra_properties.anon_distinct_id,
            session_id: self
                .extra_properties
                .session_id
                .or_else(|| self.run.clone()),
            window_id: self
                .extra_properties
                .window_id
                .or_else(|| self.run_differentiator.clone()),
            device_id: self
                .extra_properties
                .device_id
                .or_else(|| self.workflow.clone().map(DeviceId::from)),
            groups,
            properties: self.extra_properties.properties,
        }
    }
}
