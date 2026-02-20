use serde::Deserialize;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompressionSet {
    pub(crate) zstd: bool,
}

impl CompressionSet {
    pub(crate) fn delete(&mut self, algo: &CompressionAlgorithm) {
        match algo {
            CompressionAlgorithm::Identity => {
                // noop
            }
            CompressionAlgorithm::Zstd => {
                self.zstd = false;
            }
        }
    }

    pub(crate) fn into_iter(self) -> std::vec::IntoIter<CompressionAlgorithm> {
        let mut algos = Vec::with_capacity(2);
        if self.zstd {
            algos.push(CompressionAlgorithm::Zstd);
        }

        algos.push(CompressionAlgorithm::Identity);

        algos.into_iter()
    }
}

impl std::default::Default for CompressionSet {
    fn default() -> Self {
        Self { zstd: true }
    }
}

impl<'de> Deserialize<'de> for CompressionSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let algos: Vec<_> = Vec::<serde_json::Value>::deserialize(deserializer)?
            .into_iter()
            .filter_map(
                |v| match serde_json::from_value::<CompressionAlgorithm>(v) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::trace!(%e, "Unsupported compression algorithm");
                        None
                    }
                },
            )
            .collect();

        if algos.is_empty() {
            return Ok(CompressionSet { zstd: false });
        }

        let mut set = CompressionSet { zstd: false };

        for algo in algos.into_iter() {
            match algo {
                CompressionAlgorithm::Zstd => {
                    set.zstd = true;
                }
                CompressionAlgorithm::Identity => {
                    // noop
                }
            }
        }

        Ok(set)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CompressionAlgorithm {
    Identity,
    Zstd,
}

impl CompressionAlgorithm {
    pub(crate) fn content_encoding(&self) -> Option<String> {
        match self {
            CompressionAlgorithm::Identity => None,
            CompressionAlgorithm::Zstd => Some("zstd".to_string()),
        }
    }

    pub(crate) async fn compress(&self, r: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        match self {
            CompressionAlgorithm::Identity => Ok(r.into()),
            CompressionAlgorithm::Zstd => {
                let mut output: Vec<u8> = vec![];
                let mut encoder = async_compression::tokio::write::ZstdEncoder::new(&mut output);
                encoder.write_all(r).await?;
                encoder.shutdown().await?;

                Ok(output)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::CompressionSet;

    #[test]
    fn test_parse_compression_empty_defaults_to_identity() {
        let json = r#"
        [
        ]
        "#;

        assert_eq!(
            serde_json::from_str::<CompressionSet>(json).unwrap(),
            CompressionSet { zstd: false }
        );
    }

    #[test]
    fn test_parse_compression_few() {
        let json = r#"
        [
          "zstd",
          "identity"
        ]
        "#;

        assert_eq!(
            serde_json::from_str::<CompressionSet>(json).unwrap(),
            CompressionSet { zstd: true }
        );
    }

    #[test]
    fn test_parse_compression_zstd_not_identity() {
        let json = r#"
        [
          "zstd"
        ]
        "#;

        assert_eq!(
            serde_json::from_str::<CompressionSet>(json).unwrap(),
            CompressionSet { zstd: true }
        );
    }

    #[test]
    fn test_parse_compression_zstd_with_bogus() {
        let json = r#"
        [
          "zstd",
          "abc123"
        ]
        "#;

        assert_eq!(
            serde_json::from_str::<CompressionSet>(json).unwrap(),
            CompressionSet { zstd: true }
        );
    }
}
