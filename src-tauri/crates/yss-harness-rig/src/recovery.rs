//! Retry only the interrupted sampling boundary, after committed tools have settled.
use std::sync::{Arc, Mutex};
use std::time::Duration;
use yss_harness_contract::*;

pub(crate) const STREAM_IDLE: Duration = Duration::from_secs(300);

#[derive(Default)]
pub(crate) struct ModelActivity(Mutex<Option<tokio::time::Instant>>);
impl ModelActivity {
    pub(crate) fn touch(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(tokio::time::Instant::now());
    }
    pub(crate) fn finish(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    pub(crate) fn expired(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some_and(|last| last.elapsed() >= STREAM_IDLE)
    }
}

pub(crate) struct TurnOutput {
    inner: Arc<dyn AgentEventOutput>,
    text: Mutex<String>,
}
impl TurnOutput {
    pub(crate) fn new(inner: Arc<dyn AgentEventOutput>) -> Self {
        Self {
            inner,
            text: Mutex::new(String::new()),
        }
    }
    pub(crate) fn text(&self) -> String {
        self.text.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub(crate) fn position(&self) -> usize {
        self.text.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
    pub(crate) async fn rewind(&self, position: usize) -> Result<(), AgentDriverFailure> {
        let characters = {
            let text = self.text.lock().unwrap_or_else(|e| e.into_inner());
            text.get(position..)
                .ok_or_else(|| AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure))?
                .chars()
                .count()
        };
        if characters != 0 {
            self.emit(AgentEvent::TextRetracted { characters })
                .await
                .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::OutputUnavailable))?;
        }
        Ok(())
    }
}
impl AgentEventOutput for TurnOutput {
    fn emit<'a>(&'a self, event: AgentEvent) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
        Box::pin(async move {
            self.inner.emit(event.clone()).await?;
            let mut text = self.text.lock().unwrap_or_else(|e| e.into_inner());
            match event {
                AgentEvent::TextDelta { delta } => text.push_str(&delta),
                AgentEvent::TextRetracted { characters } => {
                    let keep = text.chars().count().saturating_sub(characters);
                    let boundary = text.char_indices().nth(keep).map_or(text.len(), |(i, _)| i);
                    text.truncate(boundary);
                }
                _ => {}
            }
            Ok(())
        })
    }
}

pub(crate) fn retryable(code: AgentDriverFailureCode) -> bool {
    matches!(
        code,
        AgentDriverFailureCode::ProviderTransportFailed
            | AgentDriverFailureCode::ProviderStreamInterrupted
            | AgentDriverFailureCode::ProviderRateLimited
            | AgentDriverFailureCode::ProviderUnavailable
    )
}

pub(crate) async fn backoff(
    error: &AgentDriverFailure,
    attempt: u32,
    cancellation: &CancellationToken,
) -> Result<(), AgentDriverFailure> {
    let delay = Duration::from_millis(
        error
            .retry_after_ms
            .unwrap_or(500 * (1_u64 << attempt.min(5))),
    );
    tokio::select! {
        _ = tokio::time::sleep(delay) => Ok(()),
        _ = cancellation.cancelled() => Err(crate::error::cancelled()),
    }
}
