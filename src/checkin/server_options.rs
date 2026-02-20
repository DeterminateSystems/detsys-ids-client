use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq)]
pub(crate) struct ServerOptions {
    pub(crate) compression_algorithms: crate::compression_set::CompressionSet,
}
