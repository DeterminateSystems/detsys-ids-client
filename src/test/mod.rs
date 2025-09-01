mod basic;
mod slow_transport;
mod timeout;

use once_cell::sync::Lazy;
use tracing_subscriber::fmt;

pub(crate) static TRACING: Lazy<()> = Lazy::new(|| {
    let _ = fmt().with_test_writer().try_init();
});

pub(crate) fn init_tracing() {
    Lazy::force(&TRACING);
}
