//! Periodically calls the Client pub/sub notifier routine.
use std::{process::abort, sync::Arc, time::Duration};

use futures::prelude::*;
use tokio::timer::Interval;

use crate::client::Client;

/// Return a client notifier future which will call the `Client::new_blocks`
/// method periodically.
pub fn notify_client_blocks(
    client: Arc<Client>,
    interval_secs: u64,
) -> impl Future<Item = (), Error = ()> {
    Interval::new_interval(Duration::new(interval_secs, 0))
        .for_each(move |_| {
            client.new_blocks();
            Ok(())
        })
        .map_err(|err| {
            error!("Pub/sub notifier error: {}", err);
            abort();
        })
}
