use std::{future::Future, time::Duration};

use file::FileTransport;
use http::ReqwestTransport;
use reqwest::Certificate;
use srv_http::SrvHttpTransport;
use url::Url;

use crate::{Map, submitter::Batch};

mod file;
mod http;
mod srv_http;

pub(crate) const APPLICATION_JSON: &str = "application/json";
pub(crate) trait Transport: Send + Sync + Clone + 'static {
    type Error: std::error::Error;

    fn checkin(
        &self,
        session_properties: Map,
    ) -> impl Future<Output = Result<crate::checkin::Checkin, Self::Error>> + Send;

    fn submit(&mut self, batch: Batch<'_>) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub(crate) fn default_transport_backend() -> (String, Url, Option<Vec<url::Host>>) {
    (
        "_detsys_ids._tcp.install.determinate.systems.".to_string(),
        reqwest::Url::parse("https://install.determinate.systems").unwrap(),
        Some(vec![
            url::Host::Domain(".install.determinate.systems.".into()),
            url::Host::Domain(".install.detsys.dev.".into()),
        ]),
    )
}

#[derive(Clone)]
pub(crate) enum Transports {
    None,
    File(FileTransport),
    Http(ReqwestTransport),
    SrvHttp(SrvHttpTransport),
}

impl Transports {
    pub(crate) fn none() -> Self {
        Transports::None
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(err(level = tracing::Level::TRACE)))]
    pub(crate) async fn try_new(
        opt_value: Option<String>,
        timeout: Duration,
        certificates: Option<Certificate>,
        proxy: Option<Url>,
    ) -> Result<Self, TransportsError> {
        let Some(value) = opt_value else {
            let (record, fallback, allowed_suffixes) = default_transport_backend();

            return Ok(Self::SrvHttp(SrvHttpTransport::new(
                record,
                fallback,
                allowed_suffixes,
                timeout,
                certificates,
                proxy,
            )?));
        };
        let url = Url::parse(&value).or_else(|e| {
            if e == url::ParseError::RelativeUrlWithoutBase {
                tracing::debug!("Re-parsing the URL with a file:// prefix");
                Url::parse(&format!("file://{value}"))
            } else {
                Err(e)
            }
        })?;

        match url.scheme() {
            "https" | "http" => Ok(Transports::Http(http::ReqwestTransport::new(
                url,
                timeout,
                certificates,
                proxy,
            )?)),
            "file" => Ok(Transports::File(
                FileTransport::new(
                    url.path(),
                    std::env::var_os("DETSYS_IDS_CHECKIN_FILE").map(std::path::PathBuf::from),
                )
                .await?,
            )),
            _ => Err(TransportsError::UnknownUrlScheme),
        }
    }
}

impl Transport for Transports {
    type Error = TransportsError;

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn checkin(
        &self,
        session_properties: Map,
    ) -> Result<crate::checkin::Checkin, Self::Error> {
        match self {
            Self::None => Ok(crate::checkin::Checkin {
                options: std::collections::HashMap::new(),
                ..Default::default()
            }),
            Self::File(t) => Ok(t.checkin(session_properties).await?),
            Self::Http(t) => Ok(t.checkin(session_properties).await?),
            Self::SrvHttp(t) => Ok(t.checkin(session_properties).await?),
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn submit(&mut self, batch: Batch<'_>) -> Result<(), Self::Error> {
        match self {
            Self::None => Ok(()),
            Self::File(t) => Ok(t.submit(batch).await?),
            Self::Http(t) => Ok(t.submit(batch).await?),
            Self::SrvHttp(t) => Ok(t.submit(batch).await?),
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
