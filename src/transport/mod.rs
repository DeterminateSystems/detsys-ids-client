use std::{future::Future, sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use file::FileTransport;
use http::ReqwestTransport;
use reqwest::Certificate;
use srv_http::SrvHttpTransport;
use url::Url;

use crate::{Map, submitter::Batch};

mod file;
mod http;
mod srv_http;

#[cfg(not(feature = "fips"))]
const IDS_HOST: &str = "install.determinate.systems";
#[cfg(feature = "fips")]
const IDS_HOST: &str = "install.determinate.us";
pub(crate) const APPLICATION_JSON: &str = "application/json";

pub trait Transport: Send + Sync + Clone + 'static {
    type Error: std::error::Error;

    fn checkin(
        &self,
        session_properties: Map,
    ) -> impl Future<Output = Result<crate::checkin::Checkin, Self::Error>> + Send;

    fn submit(&self, batch: Batch<'_>) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub(crate) fn default_transport_backend() -> (String, Url, Option<Vec<url::Host>>) {
    (
        format!("_detsys_ids._tcp.{IDS_HOST}.").to_string(),
        reqwest::Url::parse(&format!("https://{IDS_HOST}")).unwrap(),
        Some(vec![
            url::Host::Domain(format!(".{IDS_HOST}.")),
            #[cfg(not(feature = "fips"))]
            url::Host::Domain(".install.detsys.dev.".into()),
        ]),
    )
}

#[derive(Clone)]
pub struct Transports {
    pub transport_kind: Arc<ArcSwap<TransportKind>>,
}

#[derive(Clone)]
pub enum TransportKind {
    None,
    File(FileTransport),
    Http(ReqwestTransport),
    SrvHttp(SrvHttpTransport),
}

impl Transports {
    pub(crate) fn none() -> Self {
        Self {
            transport_kind: Arc::new(ArcSwap::from_pointee(TransportKind::None)),
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(err(level = tracing::Level::TRACE)))]
    pub(crate) async fn try_new(
        transport_url: Option<String>,
        timeout: Duration,
        certificates: Option<Certificate>,
        proxy: Option<Url>,
    ) -> Result<Self, TransportsError> {
        let transport_kind = match transport_url {
            Some(transport_url) => {
                let url = Url::parse(&transport_url).or_else(|e| {
                    if e == url::ParseError::RelativeUrlWithoutBase {
                        tracing::debug!("Re-parsing the URL with a file:// prefix");
                        Url::parse(&format!("file://{transport_url}"))
                    } else {
                        Err(e)
                    }
                })?;

                match url.scheme() {
                    "https" | "http" => TransportKind::Http(http::ReqwestTransport::new(
                        url,
                        timeout,
                        certificates,
                        proxy,
                    )?),
                    "file" => TransportKind::File(
                        FileTransport::new(
                            url.path(),
                            std::env::var_os("DETSYS_IDS_CHECKIN_FILE")
                                .map(std::path::PathBuf::from),
                        )
                        .await?,
                    ),
                    _ => return Err(TransportsError::UnknownUrlScheme),
                }
            }
            None => {
                let (record, fallback, allowed_suffixes) = default_transport_backend();

                TransportKind::SrvHttp(SrvHttpTransport::new(
                    record,
                    fallback,
                    allowed_suffixes,
                    timeout,
                    certificates,
                    proxy,
                )?)
            }
        };

        Ok(Self {
            transport_kind: Arc::new(ArcSwap::from_pointee(transport_kind)),
        })
    }
}

impl Transport for Transports {
    type Error = TransportsError;

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn checkin(
        &self,
        session_properties: Map,
    ) -> Result<crate::checkin::Checkin, Self::Error> {
        match self.transport_kind.load().as_ref() {
            TransportKind::None => Ok(crate::checkin::Checkin {
                ..Default::default()
            }),
            TransportKind::File(t) => Ok(t.checkin(session_properties).await?),
            TransportKind::Http(t) => Ok(t.checkin(session_properties).await?),
            TransportKind::SrvHttp(t) => Ok(t.checkin(session_properties).await?),
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn submit(&self, batch: Batch<'_>) -> Result<(), Self::Error> {
        match self.transport_kind.load().as_ref() {
            TransportKind::None => Ok(()),
            TransportKind::File(t) => Ok(t.submit(batch).await?),
            TransportKind::Http(t) => Ok(t.submit(batch).await?),
            TransportKind::SrvHttp(t) => Ok(t.submit(batch).await?),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TransportsError {
    #[error(transparent)]
    FileError(#[from] file::FileTransportError),

    #[error(transparent)]
    HttpError(#[from] http::ReqwestTransportError),

    #[error(transparent)]
    SrvHttpError(#[from] srv_http::SrvHttpTransportError),

    #[error("Only http, https, and file URL schemes are supported.")]
    UnknownUrlScheme,

    #[error(transparent)]
    Parse(#[from] url::ParseError),

    #[error("Read path `{0}`")]
    Read(std::path::PathBuf, #[source] std::io::Error),

    #[error("Unknown certificate format, `der` and `pem` supported")]
    UnknownCertFormat,
}
