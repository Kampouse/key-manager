use crate::FastData;
use std::future::Future;
use std::time::Duration;

/// Retry an async operation with configurable delays between attempts.
/// Returns the first successful result, or the last error if all attempts fail.
pub async fn retry_with_delays<F, Fut, T>(
    delays: &[u64],
    mut op: F,
) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    if delays.is_empty() {
        anyhow::bail!("retry_with_delays requires at least one attempt");
    }
    let mut last_error = None;
    for (attempt, &delay_secs) in delays.iter().enumerate() {
        if attempt > 0 {
            tracing::warn!(
                "Retrying (attempt {}/{}) after {}s delay",
                attempt + 1, delays.len(), delay_secs
            );
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                tracing::error!(
                    "Operation failed (attempt {}): {:?}",
                    attempt + 1, e
                );
                last_error = Some(e);
            }
        }
    }
    Err(last_error.unwrap())
}

pub fn compute_order_id(fd: &FastData) -> anyhow::Result<u64> {
    if fd.receipt_index >= 100_000 || fd.action_index >= 1_000 {
        anyhow::bail!(
            "order_id encoding overflow: receipt_index={}, action_index={} for receipt {} (max receipt_index=99999, max action_index=999)",
            fd.receipt_index, fd.action_index, fd.receipt_id
        );
    }
    Ok(((fd.shard_id as u64) * 100_000 + fd.receipt_index as u64) * 1_000 + fd.action_index as u64)
}
