use std::sync::Arc;

use detsys_srv::SrvClient;
use reqwest::Certificate;
use reqwest::Url;
use tracing::Instrument;

use crate::submitter::Batch;
use crate::Map;

use super::Transport;

type Resolver = trust_dns_resolver::AsyncResolver<
    trust_dns_resolver::name_server::GenericConnector<
        trust_dns_resolver::name_server::TokioRuntimeProvider,
    >,
>;

#[derive(Clone)]
pub(crate) struct SrvHttpTransport {
    srv: Arc<SrvClient<Resolver>>,
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

        let resolver =
            trust_dns_resolver::AsyncResolver::tokio_from_system_conf().unwrap_or_else(|e| {
                tracing::debug!(%e, "Failed to load resolv.conf settings, falling back to Google DNS.");
                trust_dns_resolver::AsyncResolver::tokio(
                    trust_dns_resolver::config::ResolverConfig::google(),
                    trust_dns_resolver::config::ResolverOpts::default(),
                )
            });

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
        })
    }
}

impl Transport for SrvHttpTransport {
    type Error = SrvHttpTransportError;

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn submit<'b>(&mut self, batch: Batch<'b>) -> Result<(), Self::Error> {
        let payload = serde_json::to_string(&batch)?;
        let reqwest = self.reqwest.clone();

        let resp = self
            .srv
            .execute(move |mut url| {
                let payload = payload.clone();
                let reqwest = reqwest.clone();

                url.set_path("/events/batch");
                let span = tracing::trace_span!("submission attempt", host = url.to_string());

                async move {
                    tracing::trace!("Submitting event logs.");

                    reqwest
                        .post(url)
                        .header(
                            http::header::CONTENT_TYPE,
                            crate::transport::APPLICATION_JSON,
                        )
                        .body(payload)
                        .send()
                        .await
                        .map_err(SrvHttpTransportError::from)
                }
                .instrument(span)
            })
            .await?;

        if resp.status().is_success() {
            return Ok(());
        }

        Err(Self::Error::Response(resp))
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn checkin(
        &self,
        session_properties: Map,
    ) -> Result<crate::checkin::Checkin, Self::Error> {
        let payload = serde_json::to_string(&session_properties)?;
        let reqwest = self.reqwest.clone();
        let resp = self
            .srv
            .execute(move |mut url| {
                let payload = payload.clone();
                let reqwest = reqwest.clone();
                url.set_path("check-in");

                let span = tracing::trace_span!("check-in attempt", host = url.to_string());

                async move {
                    tracing::trace!("Fetching check-in configuration.");

                    reqwest
                        .post(url)
                        .header(
                            http::header::CONTENT_TYPE,
                            crate::transport::APPLICATION_JSON,
                        )
                        .body(payload)
                        .send()
                        .await
                        .map_err(SrvHttpTransportError::from)
                }
                .instrument(span)
            })
            .await?;

        Ok(resp.json().await?)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SrvHttpTransportError {
    #[error(transparent)]
    SrvError(#[from] detsys_srv::Error<<Resolver as detsys_srv::resolver::SrvResolver>::Error>),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("Error with our request: {0:?}")]
    Response(reqwest::Response),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
}
