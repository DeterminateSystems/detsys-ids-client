use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;

use serde::Deserialize;

use crate::{DeviceId, DistinctId, Map};

const IDENTITY_FILE: &str = "/var/lib/determinate/identity.json";

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
        Self::import_from_env()
            .or_else(Self::import_from_file)
            .unwrap_or_default()
    }

    #[tracing::instrument]
    fn import_from_env() -> Option<Correlation> {
        let correlation = serde_json::from_slice(
            std::env::var_os("DETSYS_CORRELATION")?.as_bytes(),
        )
        .inspect_err(
            |e| tracing::trace!(%e, %IDENTITY_FILE, "DETSYS_CORRELATION contained a malformed document"),
        )
        .ok()?;

        match correlation {
            CorrelationInputs::DetSysTs(a) => Some(a.into_correlation()),
            CorrelationInputs::Direct(a) => Some(a),
        }
    }

    #[tracing::instrument]
    fn import_from_file() -> Option<Correlation> {
        let content = std::fs::read_to_string(IDENTITY_FILE)
            .inspect_err(|e| tracing::trace!(%e, %IDENTITY_FILE, "Error loading the identity file"))
            .ok()?;

        let  correlation = serde_json::from_slice(content.as_bytes())
            .inspect_err(|e| tracing::trace!(%e, %IDENTITY_FILE, "Identity file contained a malformed document"))
            .ok()?;

        match correlation {
            CorrelationInputs::DetSysTs(a) => Some(a.into_correlation()),
            CorrelationInputs::Direct(a) => Some(a),
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
