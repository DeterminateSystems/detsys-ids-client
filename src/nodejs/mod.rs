use std::sync::TryLockError;

use neon::prelude::*;
use once_cell::sync::OnceCell;
//use serde::Deserialize;
use tokio::runtime::Runtime;

use crate::{Builder, Recorder};

mod builder;
mod recorder;

// Lifted from the Neon example (licensed MIT):
// https://github.com/neon-bindings/examples/blob/7a466b2c41bdee95bca51c0d3f65343f59436fbe/examples/tokio-fetch/src/lib.rs,
//
// Return a global tokio runtime or create one if it doesn't exist.
// Throws a JavaScript exception if the `Runtime` fails to create.
fn runtime<'a, C: Context<'a>>(cx: &mut C) -> NeonResult<&'static Runtime> {
    static RUNTIME: OnceCell<Runtime> = OnceCell::new();

    RUNTIME.get_or_try_init(|| Runtime::new().or_else(|err| cx.throw_error(err.to_string())))
}

pub(crate) fn neon_hook(mut cx: ModuleContext) -> neon::result::NeonResult<()> {
    builder::neon_hook(&mut cx)?;
    recorder::neon_hook(&mut cx)?;

    Ok(())
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("Could not lock the resource: {0}")]
    Lock(String),

    #[error("Invalid integer: {0}")]
    FromInt(#[from] std::num::TryFromIntError),
}

impl<T> From<TryLockError<T>> for Error {
    fn from(err: TryLockError<T>) -> Self {
        Self::Lock(err.to_string())
    }
}

impl Finalize for Builder {}
impl Finalize for Recorder {}
