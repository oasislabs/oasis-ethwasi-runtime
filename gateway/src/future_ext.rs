//! Future extensions.
use std::sync::mpsc;

use futures::prelude::*;
use tokio::runtime::TaskExecutor;

/// Spawn a future in our environment and wait for its result.
pub fn block_on<F, R, E>(executor: &TaskExecutor, future: F) -> Result<R, E>
where
    F: Send + 'static + Future<Item = R, Error = E>,
    R: Send + 'static,
    E: Send + 'static,
{
    let (result_tx, result_rx) = mpsc::channel();
    executor.spawn(future.then(move |result| {
        drop(result_tx.send(result));
        Ok(())
    }));

    result_rx
        .recv()
        .expect("block_on: executor dropped our result sender")
}
