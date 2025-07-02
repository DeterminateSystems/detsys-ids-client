use reqwest::Certificate;
use url::Url;

use crate::{Map, submitter::Batch};

use super::Transport;

#[derive(Clone)]
pub(crate) struct ReqwestTransport {
    host: Url,
    timeout: std::time::Duration,
    client: reqwest::Client,
}
impl ReqwestTransport {
    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(err))]
    pub(crate) fn new(
        host: Url,
        timeout: std::time::Duration,
        certificates: Option<Certificate>,
        proxy: Option<Url>,
    ) -> Result<Self, ReqwestTransportError> {
        let mut builder = reqwest::ClientBuilder::new();

        if let Some(cert) = certificates {
            builder = builder.add_root_certificate(cert);
        }

        if let Some(proxy) = proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy.clone())?);
        }

        Ok(ReqwestTransport {
            host,
            client: builder.build()?,
            timeout,
        })
    }
}

impl Transport for ReqwestTransport {
    type Error = ReqwestTransportError;

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn submit(&mut self, batch: Batch<'_>) -> Result<(), Self::Error> {
        let mut url = self.host.clone();
        url.set_path("/events/batch");

        let resp = self
            .client
            .post(url)
            .timeout(self.timeout)
            .json(&batch)
            .send()
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
        let mut url = self.host.clone();
        url.set_path("/check-in");

        let res = self
            .client
            .post(url.clone())
            .json(&session_properties)
            .timeout(self.timeout)
            .send()
            .await;

        match res {
            Ok(resp) => Ok(resp.json().await?),
            Err(err) => {
                tracing::debug!("Failed to check in with `{url}`, continuing");
                Err(err)?
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ReqwestTransportError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("Error with our request: {0:?}")]
    Response(reqwest::Response),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}
