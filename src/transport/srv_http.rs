use std::sync::Arc;

use detsys_srv::SrvClient;
use reqwest::Certificate;
use reqwest::Url;
use tracing::Instrument;

use crate::Map;
use crate::checkin::Checkin;
use crate::checkin::ServerOptions;
use crate::submitter::Batch;

use super::Transport;

type Resolver = hickory_resolver::TokioResolver;
// type Resolver = hickory_resolver::AsyncResolver<
//     hickory_resolver::name_server::GenericConnector<
//         hickory_resolver::name_server::TokioRuntimeProvider,
//     >,
// >;

#[derive(Clone)]
pub(crate) struct SrvHttpTransport {
    srv: Arc<SrvClient<Resolver>>,
    server_options: Arc<tokio::sync::RwLock<crate::checkin::ServerOptions>>,
    reqwest: reqwest::Client,
}
impl SrvHttpTransport {
    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(err(level = tracing::Level::TRACE)))]
    pub(crate) fn new(
        record: impl Into<String> + std::fmt::Debug,
        fallback: impl Into<Url> + std::fmt::Debug,
        allowed_suffixes: Option<Vec<url::Host>>,
        timeout: std::time::Duration,
        certificates: Option<Certificate>,
        proxy: Option<Url>,
    ) -> Result<SrvHttpTransport, SrvHttpTransportError> {
        let record = record.into();
        let fallback = fallback.into();

        let resolver = hickory_resolver::TokioResolver::builder_tokio().unwrap_or_else(|e| {
            tracing::debug!(%e, "Failed to load resolv.conf settings, falling back to Google DNS.");
            hickory_resolver::Resolver::builder_with_config(
                hickory_resolver::config::ResolverConfig::google(),
                hickory_resolver::name_server::TokioConnectionProvider::default(),
            )
        }).build();

        let srv =
            SrvClient::<Resolver>::new_with_resolver(&record, fallback, allowed_suffixes, resolver);

        let mut builder = reqwest::ClientBuilder::new().timeout(timeout);

        if let Some(cert) = certificates {
            builder = builder.add_root_certificate(cert);
        }

        if let Some(proxy) = proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy.clone())?);
        }

        Ok(SrvHttpTransport {
            srv: Arc::new(srv),
            reqwest: builder.build()?,
            server_options: Arc::new(tokio::sync::RwLock::new(
                crate::checkin::ServerOptions::default(),
            )),
        })
    }
}

impl Transport for SrvHttpTransport {
    type Error = SrvHttpTransportError;

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn submit(&mut self, batch: Batch<'_>) -> Result<(), Self::Error> {
        let payload = serde_json::to_string(&batch)?;
        let reqwest = self.reqwest.clone();
        let server_opts = self.server_options.clone();

        let resp = self
            .srv
            .execute(move |mut url| {
                let payload: Vec<u8> = payload.as_bytes().into();
                let reqwest = reqwest.clone();
                let server_opts = server_opts.clone();

                url.set_path("/events/batch");

                let span = tracing::debug_span!("submission", %url);

                perform_request(reqwest, url, payload, server_opts).instrument(span)
            })
            .await?;

        if resp.status().is_success() {
            return Ok(());
        }

        Err(Self::Error::Response(Box::new(resp)))
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn checkin(
        &self,
        session_properties: Map,
    ) -> Result<crate::checkin::Checkin, Self::Error> {
        let payload = serde_json::to_string(&session_properties)?;
        let reqwest = self.reqwest.clone();
        let server_opts = self.server_options.clone();

        let resp = self
            .srv
            .execute(move |mut url| {
                let payload: Vec<u8> = payload.as_bytes().into();
                let reqwest = reqwest.clone();
                let server_opts = server_opts.clone();

                url.set_path("check-in");

                let span = tracing::trace_span!("check-in attempt", %url);

                perform_request(reqwest, url, payload, server_opts).instrument(span)
            })
            .await?;

        let checkin: Checkin = resp.json().await?;

        // Update server options to sync up compression options
        {
            let mut opts = self.server_options.write().await;
            *opts = checkin.server_options.clone();
        }

        Ok(checkin)
    }
}

#[tracing::instrument(skip(reqwest, payload, server_opts))]
async fn perform_request(
    reqwest: reqwest::Client,
    url: url::Url,
    payload: Vec<u8>,
    server_opts: Arc<tokio::sync::RwLock<ServerOptions>>,
) -> Result<reqwest::Response, SrvHttpTransportError> {
    let algos = server_opts.read().await.compression_algorithms.into_iter();

    for compression_algo in algos {
        let span = tracing::debug_span!("requesting", ?compression_algo);

        let mut req = reqwest
            .post(url.clone())
            .header(
                http::header::CONTENT_TYPE,
                crate::transport::APPLICATION_JSON,
            )
            .body(compression_algo.compress(&payload).await?);

        if let Some(encoding) = compression_algo.content_encoding() {
            req = req.header(http::header::CONTENT_ENCODING, encoding);
        }

        tracing::trace!(parent: &span, "Requesting");
        match req.send().instrument(span.clone()).await {
            Ok(resp) if resp.status() == http::StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                tracing::debug!(
                    ?compression_algo,
                    "Disabling compression algorithm because it is unsupported"
                );
                server_opts
                    .write()
                    .await
                    .compression_algorithms
                    .delete(&compression_algo);
            }

            Err(e) => {
                return Err(SrvHttpTransportError::from(e));
            }
            Ok(resp) => return Ok(resp),
        }
    }

    Err(SrvHttpTransportError::NoCompressionMode)
}

#[derive(thiserror::Error, Debug)]
pub enum SrvHttpTransportError {
    #[error(transparent)]
    SrvError(#[from] detsys_srv::Error<<Resolver as detsys_srv::resolver::SrvResolver>::Error>),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("Error with our request: {0:?}")]
    Response(Box<reqwest::Response>),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    UrlParse(#[from] url::ParseError),

    #[error("The server has rejected all of our compression modes")]
    NoCompressionMode,
}
