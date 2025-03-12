#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AnonymousDistinctId(String);

impl AnonymousDistinctId {
    pub fn new() -> AnonymousDistinctId {
        AnonymousDistinctId(uuid::Uuid::now_v7().to_string())
    }
}

impl Default for AnonymousDistinctId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for AnonymousDistinctId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for AnonymousDistinctId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Eq)]
pub struct DistinctId(String);

impl From<String> for DistinctId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for DistinctId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Eq)]
pub struct DeviceId(String);

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceId {
    pub fn new() -> DeviceId {
        DeviceId(format!("DIDS-DEV-{}", uuid::Uuid::now_v7()))
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)
    }
}

impl From<String> for DeviceId {
    fn from(value: String) -> Self {
        Self(value)
    }
}
