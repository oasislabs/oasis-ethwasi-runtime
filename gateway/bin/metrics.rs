//! Prometheus metrics.
use std::{net, thread, time::Duration};

use ekiden_runtime::common::logger::get_logger;
use prometheus::{self, labels};
use slog::{info, warn, Logger};

/// Metrics service configuration.
pub enum Config {
    Push {
        address: String,
        period: Duration,
        job_name: String,
        instance_name: String,
    },
    Pull {
        address: net::SocketAddr,
    },
}

/// Pushes metrics to Prometheus pushgateway.
fn push_metrics(logger: Logger, address: &str, job_name: &str, instance_name: &str) {
    prometheus::push_metrics(
        job_name,
        labels! {"instance".to_owned() => instance_name.to_owned(),},
        address,
        prometheus::gather(),
    )
    .or_else::<prometheus::Error, _>(|err| {
        warn!(logger, "Cannot push Prometheus metrics"; "err" => ?err);
        Ok(())
    })
    .unwrap();
}

/// Start a thread for serving or pushing Prometheus metrics.
pub fn start(cfg: Config) {
    thread::spawn(move || {
        let logger = get_logger("gateway/metrics");
        info!(logger, "Starting Prometheus metrics thread");

        match cfg {
            Config::Push {
                address,
                period,
                job_name,
                instance_name,
            } => {
                info!(logger, "Configured in push mode";
                    "address" => &address,
                    "period" => ?period,
                    "job_name" => &job_name,
                    "instance_name" => &instance_name,
                );

                loop {
                    // Sleep for the given period.
                    thread::sleep(period);

                    // Try to push metrics.
                    push_metrics(logger.clone(), &address, &job_name, &instance_name);
                }
            }
            Config::Pull { .. } => {
                // TODO: Pull.
            }
        }
    });
}
