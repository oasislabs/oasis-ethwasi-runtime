//! Periodically calls the Client pub/sub notifier routine.

use std::{process::abort, sync::Arc, time::Duration};

use ekiden_common::{
    futures::{killable, KillHandle},
    tokio::timer::Interval,
};
use ekiden_core::{environment::Environment, futures::FutureExt};
use futures::{Future, Stream};

use client::Client;

pub struct PubSubNotifier {
    client: Arc<Client>,
    environment: Arc<Environment>,
    interval_secs: u64,
    notifier_task: Option<KillHandle>,
}

impl PubSubNotifier {
    pub fn new(client: Arc<Client>, environment: Arc<Environment>, interval_secs: u64) -> Self {
        let mut instance = Self {
            client: client.clone(),
            environment: environment.clone(),
            interval_secs,
            notifier_task: None,
        };
        instance.start();
        instance
    }

    fn start(&mut self) {
        let interval = Interval::new_interval(Duration::new(self.interval_secs, 0));
        let (f, kill_handle) = killable({
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
        });
        self.environment.spawn(f.map(|_| ()).into_box());
        self.notifier_task = Some(kill_handle);
    }
}

impl Drop for PubSubNotifier {
    fn drop(&mut self) {
        // Kill the notifier task.
        if let Some(task) = self.notifier_task.take() {
            task.kill();
        }
    }
}
