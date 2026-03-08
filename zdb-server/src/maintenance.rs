use std::time::Duration;

use crate::actor::ActorHandle;

/// Periodically runs compaction + stale node detection.
pub async fn maintenance_loop(actor: ActorHandle, interval_secs: u64) {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    // Skip the first immediate tick
    interval.tick().await;

    loop {
        interval.tick().await;
        log::debug!("maintenance: starting scheduled run");
        match actor.run_maintenance(false).await {
            Ok(_report) => log::debug!("maintenance: completed"),
            Err(e) => log::warn!("maintenance: failed: {e}"),
        }
    }
}
