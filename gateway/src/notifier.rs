//! Periodically calls the Client notifier routine.

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
}

impl PubSubNotifier {
    pub fn new(client: Arc<Client>, environment: Arc<Environment>) -> Self {
        let instance = Self {
            client: client.clone(),
            environment: environment.clone(),
        };
        instance.start();
        instance
    }

    fn start(&self) {
        const INTERVAL_SECS: u64 = 5;

        let interval = Interval::new_interval(Duration::new(INTERVAL_SECS, 0));

        self.environment.spawn({
            let client = self.client.clone();
            interval
                .for_each(move |_instant| {
                    client.new_blocks();
                    Ok(())
                })
                .map_err(|e| error!("Notifier error: {:?}", e))
                .into_box()
        });
    }
}
