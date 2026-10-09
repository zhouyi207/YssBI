//! Provider-reported consumption for one turn, independent of its rendered history.
use yss_harness_contract::{AssistantEventKind as Event, ModelCallPurpose};

#[derive(Clone, Default)]
pub(in crate::assistant) struct TurnUsage {
    pub calls: u64,
    pub incomplete: bool,
    // Each event contains u64 counters; accumulation must retain their full precision.
    pub total: [Option<u128>; 5],
    pub latest: Option<ContextUsage>,
}

#[derive(Clone, Copy)]
pub(in crate::assistant) struct ContextUsage {
    pub input: Option<u64>,
    pub capacity: Option<u32>,
}

impl TurnUsage {
    pub fn accept(&mut self, event: &Event, worker: bool) {
        match event {
            Event::UsageReported {
                usage,
                context_window,
                purpose,
            } => {
                self.calls += 1;
                self.incomplete |= usage.input_tokens.is_none() || usage.output_tokens.is_none();
                for (total, reported) in self.total.iter_mut().zip([
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.cached_input_tokens,
                    usage.cache_creation_input_tokens,
                    usage.reasoning_tokens,
                ]) {
                    if let Some(reported) = reported {
                        *total = Some(total.unwrap_or(0) + u128::from(reported));
                    }
                }
                if !worker && *purpose == ModelCallPurpose::Response {
                    self.latest = Some(ContextUsage {
                        input: usage.input_tokens,
                        capacity: context_window.filter(|capacity| *capacity > 0),
                    });
                }
            }
            Event::ContextCompacted { .. } if !worker => self.latest = None,
            _ => {}
        }
    }
}
