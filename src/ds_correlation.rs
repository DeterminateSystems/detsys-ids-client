use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;

use serde::Deserialize;

use crate::{DeviceId, DistinctId, Map};

const IDENTITY_FILE: &str = "/var/lib/determinate/identity.json";

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
struct DetsysTsGitHubAction {
    // approximately the distinct_id / user identifier
    repository: String,

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
                .or_else(|| Some(DistinctId::from(self.repository))),
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

#[cfg(test)]
mod tests {
    use crate::ds_correlation::{Correlation, CorrelationInputs, DetsysTsGitHubAction};

    // In https://github.com/DeterminateSystems/detsys-ts/pull/104 we stopped doing the wacky transformations.
    // In that PR, we also changed the structure of the correlation we write to the identity.json to be more straightforward.
    // This test makes sure the old style parses correctly as the old style
    #[test]
    fn test_parse_detsysts_pre_104() {
        let c: CorrelationInputs = serde_json::from_value(serde_json::json!({
            "correlation_source": "github-actions",
            "repository": "GHR-xxx",
            "workflow": "GHW-xxx",
            "job": "GHWJ-xxx",
            "run": "GHWJR-xxx",
            "run_differentiator": "GHWJA-xxx",
            "groups": {
                "ci": "github-actions",
                "project": "nix-installer",
                "github_organization": "GHO-xxx"
            },
            "is_ci": true
        }))
        .unwrap();
        assert_eq!(
            c,
            CorrelationInputs::DetSysTs(DetsysTsGitHubAction {
                repository: "GHR-xxx".into(),
                run: Some("GHWJR-xxx".into()),
                run_differentiator: Some("GHWJA-xxx".into()),
                workflow: Some("GHW-xxx".into()),
                groups: std::collections::HashMap::from_iter(
                    [
                        (
                            "github_organization".to_string(),
                            Some("GHO-xxx".to_string())
                        ),
                        ("project".to_string(), Some("nix-installer".to_string())),
                        ("ci".to_string(), Some("github-actions".to_string())),
                    ]
                    .into_iter()
                ),
                extra_properties: Correlation {
                    distinct_id: None,
                    anon_distinct_id: None,
                    session_id: None,
                    window_id: None,
                    device_id: None,
                    groups: std::collections::HashMap::new(),
                    properties: super::Map::from_iter(
                        [
                            (
                                "correlation_source".to_string(),
                                serde_json::Value::from("github-actions")
                            ),
                            ("is_ci".to_string(), serde_json::Value::from(true)),
                            ("job".to_string(), serde_json::Value::from("GHWJ-xxx"))
                        ]
                        .into_iter()
                    ),
                },
            })
        );
    }

    #[test]
    fn test_parse_detsysts_post_104() {
        let c: CorrelationInputs = serde_json::from_value(serde_json::json!({
            "$anon_distinct_id": "github_xxx",
            "correlation_source": "github-actions",
            "github_repository_hash": "GHR-xxx",
            "github_workflow_hash": "GHW-xxx",
            "github_workflow_job_hash": "GHWJ-xxx",
            "github_workflow_run_hash": "GHWJR-xxx",
            "github_workflow_run_differentiator_hash": "GHWJA-xxx",
            "$session_id": "GHWJA-xxx",
            "$groups": {
                "github_repository": "GHR-xxx",
                "github_organization": "GHO-xxx"
            },
            "is_ci": true
        }))
        .unwrap();
        assert_eq!(
            c,
            CorrelationInputs::Direct(Correlation {
                anon_distinct_id: Some("github_xxx".into()),

                distinct_id: None,
                session_id: Some("GHWJA-xxx".into()),
                window_id: None,
                device_id: None,

                groups: std::collections::HashMap::from_iter(
                    [
                        (
                            "github_organization".to_string(),
                            Some("GHO-xxx".to_string())
                        ),
                        ("github_repository".to_string(), Some("GHR-xxx".to_string())),
                    ]
                    .into_iter()
                ),
                properties: super::Map::from_iter(
                    [
                        (
                            "correlation_source".to_string(),
                            serde_json::Value::from("github-actions")
                        ),
                        ("is_ci".to_string(), serde_json::Value::from(true)),
                        ("github_workflow_run_hash".to_string(), "GHWJR-xxx".into()),
                        (
                            "github_workflow_run_differentiator_hash".to_string(),
                            "GHWJA-xxx".into()
                        ),
                        ("github_workflow_hash".to_string(), "GHW-xxx".into()),
                        ("github_repository_hash".to_string(), "GHR-xxx".into()),
                        ("github_workflow_job_hash".to_string(), "GHWJ-xxx".into())
                    ]
                    .into_iter()
                ),
            })
        );
    }
}
