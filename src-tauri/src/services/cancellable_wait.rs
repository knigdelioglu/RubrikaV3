use std::future::Future;
use tokio_util::sync::CancellationToken;

/// Awaits a future while giving an optional job cancellation token priority.
///
/// Returning `None` means cancellation won the race and the caller must stop
/// the operation without committing the in-flight result.
pub async fn await_with_cancellation<F, T>(
    token: Option<&CancellationToken>,
    future: F,
) -> Option<T>
where
    F: Future<Output = T>,
{
    match token {
        Some(token) => {
            tokio::select! {
                biased;
                _ = token.cancelled() => None,
                result = future => Some(result),
            }
        }
        None => Some(future.await),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_token_drops_pending_wait_immediately() {
        let token = CancellationToken::new();
        token.cancel();

        let result = await_with_cancellation(
            Some(&token),
            std::future::pending::<u32>(),
        )
        .await;

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn completed_future_passes_through_when_not_cancelled() {
        let token = CancellationToken::new();
        let result = await_with_cancellation(Some(&token), async { 42u32 }).await;
        assert_eq!(result, Some(42));
    }

    #[tokio::test]
    async fn missing_token_preserves_normal_await_behavior() {
        let result = await_with_cancellation(None, async { "done" }).await;
        assert_eq!(result, Some("done"));
    }
}
