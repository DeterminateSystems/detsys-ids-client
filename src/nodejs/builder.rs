use std::sync::{Arc, Mutex};

use neon::prelude::*;
//use serde::Deserialize;

use crate::Builder;

use super::Error;

pub(crate) fn neon_hook(cx: &mut ModuleContext) -> neon::result::NeonResult<()> {
    cx.export_function("builderNew", Builder::js_new)?;
    cx.export_function(
        "builderSetAnonymousDistinctId",
        Builder::js_set_anonymous_distinct_id,
    )?;
    cx.export_function(
        "builderSetDistinctId",
        Builder::js_set_distinct_id,
    )?;
    cx.export_function(
        "builderSetDeviceId",
        Builder::js_set_device_id,
    )?;
    cx.export_function(
        "builderSetEndpoint",
        Builder::js_set_endpoint,
    )?;
    cx.export_function(
        "builderSetEnableReporting",
        Builder::js_set_enable_reporting,
    )?;
    cx.export_function(
        "builderSetTimeoutMs",
        Builder::js_set_timeout_ms,
    )?;
    cx.export_function(
        "builderSetFact",
        Builder::js_set_fact,
    )?;
    cx.export_function(
        "builderBuild",
        Builder::js_build,
    )?;

    Ok(())
}


type JsBuilder = JsBox<Arc<Mutex<Builder>>>;

impl Builder {
    fn js_new(mut cx: FunctionContext) -> JsResult<JsBuilder> {
        Ok(cx.boxed(Arc::new(Mutex::new(Builder::new()))))
    }

    fn js_set_anonymous_distinct_id(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let binding = cx.this::<JsBuilder>()?;
        let mut builder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

        let v: Option<String> = match cx.argument_opt(1) {
            Some(v) => Some(v.downcast_or_throw::<JsString, _>(&mut cx)?.value(&mut cx)),
            None => None,
        };

        builder.set_anonymous_distinct_id(v.map(crate::AnonymousDistinctId::from));

        Ok(cx.undefined())
    }

    fn js_set_distinct_id(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let binding = cx.this::<JsBuilder>()?;
        let mut builder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

        let v: Option<String> = match cx.argument_opt(1) {
            Some(v) => Some(v.downcast_or_throw::<JsString, _>(&mut cx)?.value(&mut cx)),
            None => None,
        };

        builder.set_distinct_id(v.map(crate::DistinctId::from));

        Ok(cx.undefined())
    }

    fn js_set_device_id(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let binding = cx.this::<JsBuilder>()?;
        let mut builder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

        let v: Option<String> = match cx.argument_opt(1) {
            Some(v) => Some(v.downcast_or_throw::<JsString, _>(&mut cx)?.value(&mut cx)),
            None => None,
        };

        builder.set_device_id(v.map(crate::DeviceId::from));

        Ok(cx.undefined())
    }

    fn js_set_endpoint(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let binding = cx.this::<JsBuilder>()?;
        let mut builder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

        let v: Option<String> = match cx.argument_opt(1) {
            Some(v) => Some(v.downcast_or_throw::<JsString, _>(&mut cx)?.value(&mut cx)),
            None => None,
        };

        builder.set_endpoint(v);

        Ok(cx.undefined())
    }

    fn js_set_enable_reporting(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let binding = cx.this::<JsBuilder>()?;
        let mut builder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

        let v: bool = cx.argument::<JsBoolean>(1)?.value(&mut cx);

        builder.set_enable_reporting(v);

        Ok(cx.undefined())
    }

    fn js_set_timeout_ms(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let binding = cx.this::<JsBuilder>()?;
        let mut builder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

        let v: Option<std::time::Duration> = match cx.argument_opt(1) {
            Some(v) => {
                let millis_f64: f64 = v.downcast_or_throw::<JsNumber, _>(&mut cx)?.value(&mut cx);
                let millis_i64 = millis_f64 as i32;
                let millis: u64 = millis_i64
                    .try_into()
                    .map_err(Error::from)
                    .or_else(|err| cx.throw_error(err.to_string()))?;

                Some(std::time::Duration::from_millis(millis))
            }
            None => None,
        };

        builder.set_timeout(v);

        Ok(cx.undefined())
    }

    fn js_set_fact(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let binding = cx.this::<JsBuilder>()?;
        let mut builder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

        let key: String = cx.argument::<JsString>(1)?.value(&mut cx);
        let value: String = cx.argument::<JsString>(2)?.value(&mut cx);

        builder.set_fact(key, value);

        Ok(cx.undefined())
    }

    fn js_build(mut cx: FunctionContext) -> JsResult<JsPromise> {
        let rt = super::runtime(&mut cx)?;

        let binding = cx.this::<JsBuilder>()?;
        let builder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

            let bod = builder.clone().build_or_default();
        let channel = cx.channel();
        let (deferred, promise) = cx.promise();

        rt.spawn(async move {
            let (recorder, worker) = bod.await;
            rt.spawn(worker.wait());

            deferred.settle_with(&channel, move |mut cx| {
                Ok(cx.boxed(recorder))
            })
        });

        Ok(promise)
    }
}

/*
    pub fn set_certificate(mut self, certificate: Option<Certificate>) -> Self {
        self.certificate = certificate;
        self
    }

    pub fn set_proxy(mut self, proxy: Option<Url>) -> Self {
        self.proxy = proxy;
        self
    }

    #[tracing::instrument(skip(self))]
    pub async fn build_or_default(mut self) -> (Recorder, Worker) {
        let transport = self.transport_or_default().await;

        self.build_with(
            transport,
            crate::system_snapshot::Generic::default(),
            crate::storage::Generic::default(),
        )
        .await
    }
}
 */
