//! Periodically calls the Client pub/sub notifier routine.

use std::process::abort;
use std::sync::Arc;
use std::time::Duration;

use ekiden_common::tokio::timer::Interval;
use ekiden_core::environment::Environment;
use ekiden_core::futures::FutureExt;
use futures::{Future, Stream};

use client::Client;

pub struct PubSubNotifier {
    client: Arc<Client>,
    environment: Arc<Environment>,
    interval_secs: u64,
}

impl PubSubNotifier {
    pub fn new(client: Arc<Client>, environment: Arc<Environment>, interval_secs: u64) -> Self {
        let instance = Self {
            client: client.clone(),
            environment: environment.clone(),
            interval_secs,
        };
        instance.start();
        instance
    }

    fn start(&self) {
        let interval = Interval::new_interval(Duration::new(self.interval_secs, 0));
        self.environment.spawn({
            let client = self.client.clone();
            interval
                .for_each(move |_| {
                    client.new_blocks();
                    Ok(())
                })
                .map_err(|e| {
                    error!("Pub/sub notifier error: {}", e);
                    abort();
                })
                .into_box()
        });
    }
}
