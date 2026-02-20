use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq)]
pub(crate) struct ServerOptions {
    pub(crate) compression_algorithms: crate::compression_set::CompressionSet,
}

impl ServerOptions {
    pub(crate) fn diff(&self, prev: &Self) -> Vec<String> {
        if self == prev {
            return vec![];
        }

        vec![format!(
            "Compression algorithms: {:?} -> {:?}",
            prev.compression_algorithms, self.compression_algorithms
        )]
    }
}

#[cfg(test)]
mod tests {
    use crate::compression_set::CompressionSet;

    use super::*;

    fn server_options(zstd: bool) -> ServerOptions {
        ServerOptions {
            compression_algorithms: CompressionSet { zstd },
        }
    }

    #[test]
    fn matching_no_diff() {
        let prev = server_options(false);
        let next = server_options(false);

        assert!(next.diff(&prev).is_empty())
    }

    #[test]
    fn diff_sensible() {
        let prev = server_options(false);
        let next = server_options(true);

        assert_eq!(
            next.diff(&prev),
            vec![String::from(
                "Compression algorithms: CompressionSet { zstd: false } -> CompressionSet { zstd: true }"
            )]
        )
    }
}
