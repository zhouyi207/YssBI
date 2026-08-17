use super::trace_bundle::{
    build_compilation_bundle, build_run_bundle, estimate_run_bundle, estimate_span,
    is_top_level_run_root,
};
use super::{
    RunId, RunTraceBundle, SYSTEM_TRACE_CLOCK, SpanGuard, SpanId, SpanKind, SpanSpec, TraceBundle,
    TraceClock, TraceSink, TraceSpan,
};
use crate::node_system::document::GraphResourcePath;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex};

pub const DEFAULT_COMPLETED_RUN_TRACE_LIMIT: usize = 32;
pub const DEFAULT_PROJECT_TRACE_BYTE_LIMIT: usize = 2 * 1024 * 1024;
pub const DEFAULT_ACTIVE_TRACE_SPAN_LIMIT: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceRetentionPolicy {
    max_completed_runs: usize,
    max_estimated_bytes: usize,
    max_active_spans_per_bundle: usize,
}

impl TraceRetentionPolicy {
    pub fn new(
        max_completed_runs: usize,
        max_estimated_bytes: usize,
    ) -> Result<Self, TraceRetentionError> {
        let policy = Self {
            max_completed_runs,
            max_estimated_bytes,
            max_active_spans_per_bundle: DEFAULT_ACTIVE_TRACE_SPAN_LIMIT,
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn with_max_active_spans_per_bundle(
        mut self,
        max_active_spans_per_bundle: usize,
    ) -> Result<Self, TraceRetentionError> {
        self.max_active_spans_per_bundle = max_active_spans_per_bundle;
        self.validate()?;
        Ok(self)
    }

    pub const fn max_completed_runs(self) -> usize {
        self.max_completed_runs
    }

    pub const fn max_estimated_bytes(self) -> usize {
        self.max_estimated_bytes
    }

    pub const fn max_active_spans_per_bundle(self) -> usize {
        self.max_active_spans_per_bundle
    }

    fn validate(self) -> Result<(), TraceRetentionError> {
        if self.max_completed_runs == 0
            || self.max_estimated_bytes == 0
            || self.max_active_spans_per_bundle == 0
        {
            return Err(TraceRetentionError);
        }
        Ok(())
    }
}

impl Default for TraceRetentionPolicy {
    fn default() -> Self {
        Self {
            max_completed_runs: DEFAULT_COMPLETED_RUN_TRACE_LIMIT,
            max_estimated_bytes: DEFAULT_PROJECT_TRACE_BYTE_LIMIT,
            max_active_spans_per_bundle: DEFAULT_ACTIVE_TRACE_SPAN_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceRetentionError;

impl fmt::Display for TraceRetentionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("trace retention and active span limits must be greater than zero")
    }
}

impl std::error::Error for TraceRetentionError {}

struct ActiveTrace {
    root_span_id: SpanId,
    spans: Vec<TraceSpan>,
    child_counts: HashMap<SpanId, usize>,
    estimated_span_bytes: u64,
    dropped_span_count: u64,
}

impl ActiveTrace {
    fn new(root_span_id: SpanId) -> Self {
        Self {
            root_span_id,
            spans: Vec::new(),
            child_counts: HashMap::new(),
            estimated_span_bytes: 0,
            dropped_span_count: 0,
        }
    }

    fn record(&mut self, span: TraceSpan, policy: TraceRetentionPolicy) {
        let span_bytes = estimate_span(&span);
        let is_root = span.span_id == self.root_span_id;
        let is_retained_parent = self.child_count(span.span_id) > 0;
        if !is_root && !self.fits_with(span_bytes, policy) && !is_retained_parent {
            self.dropped_span_count = self.dropped_span_count.saturating_add(1);
            return;
        }

        self.insert(span, span_bytes);
        self.enforce_budget(policy);
    }

    fn fits_with(&self, span_bytes: u64, policy: TraceRetentionPolicy) -> bool {
        self.spans.len().saturating_add(1) <= policy.max_active_spans_per_bundle
            && self.estimated_span_bytes.saturating_add(span_bytes)
                <= policy.max_estimated_bytes as u64
    }

    fn insert(&mut self, span: TraceSpan, span_bytes: u64) {
        if let Some(parent_span_id) = span.parent_span_id {
            *self.child_counts.entry(parent_span_id).or_default() += 1;
        }
        self.estimated_span_bytes = self.estimated_span_bytes.saturating_add(span_bytes);
        self.spans.push(span);
    }

    fn enforce_budget(&mut self, policy: TraceRetentionPolicy) {
        while self.over_budget(policy) {
            if let Some(index) = self.spans.iter().rposition(|span| {
                span.span_id != self.root_span_id && self.child_count(span.span_id) == 0
            }) {
                self.remove_leaf(index);
            } else if !self.remove_non_root_subtree() {
                break;
            }
        }
    }

    fn over_budget(&self, policy: TraceRetentionPolicy) -> bool {
        self.spans.len() > policy.max_active_spans_per_bundle
            || self.estimated_span_bytes > policy.max_estimated_bytes as u64
    }

    fn child_count(&self, span_id: SpanId) -> usize {
        self.child_counts.get(&span_id).copied().unwrap_or(0)
    }

    fn remove_leaf(&mut self, index: usize) {
        let span = self.spans.swap_remove(index);
        debug_assert_eq!(self.child_count(span.span_id), 0);
        self.child_counts.remove(&span.span_id);
        if let Some(parent_span_id) = span.parent_span_id {
            let remove_parent_entry =
                if let Some(count) = self.child_counts.get_mut(&parent_span_id) {
                    *count = count.saturating_sub(1);
                    *count == 0
                } else {
                    false
                };
            if remove_parent_entry {
                self.child_counts.remove(&parent_span_id);
            }
        }
        self.estimated_span_bytes = self
            .estimated_span_bytes
            .saturating_sub(estimate_span(&span));
        self.dropped_span_count = self.dropped_span_count.saturating_add(1);
    }

    fn remove_non_root_subtree(&mut self) -> bool {
        let Some(seed) = self
            .spans
            .iter()
            .rev()
            .find(|span| span.span_id != self.root_span_id)
            .map(|span| span.span_id)
        else {
            return false;
        };
        let mut removed_ids = HashSet::from([seed]);
        loop {
            let previous_len = removed_ids.len();
            for span in &self.spans {
                if span
                    .parent_span_id
                    .is_some_and(|parent| removed_ids.contains(&parent))
                {
                    removed_ids.insert(span.span_id);
                }
            }
            if removed_ids.len() == previous_len {
                break;
            }
        }

        let original = std::mem::take(&mut self.spans);
        let mut removed_count = 0_u64;
        self.spans = original
            .into_iter()
            .filter(|span| {
                let retain = !removed_ids.contains(&span.span_id);
                if !retain {
                    removed_count = removed_count.saturating_add(1);
                }
                retain
            })
            .collect();
        self.dropped_span_count = self.dropped_span_count.saturating_add(removed_count);
        self.rebuild_accounting();
        true
    }

    fn rebuild_accounting(&mut self) {
        self.child_counts.clear();
        self.estimated_span_bytes = 0;
        for span in &self.spans {
            if let Some(parent_span_id) = span.parent_span_id {
                *self.child_counts.entry(parent_span_id).or_default() += 1;
            }
            self.estimated_span_bytes = self
                .estimated_span_bytes
                .saturating_add(estimate_span(span));
        }
    }

    fn into_parts(self) -> (Vec<TraceSpan>, u64) {
        (self.spans, self.dropped_span_count)
    }
}

#[derive(Default)]
struct TraceStoreState {
    active_runs: HashMap<RunId, ActiveTrace>,
    active_compilations: HashMap<SpanId, ActiveTrace>,
    completed: VecDeque<TraceBundle>,
    completed_run_count: usize,
    estimated_bytes: u64,
}

pub struct BoundedTraceSink {
    policy: TraceRetentionPolicy,
    clock: Arc<dyn TraceClock>,
    state: Mutex<TraceStoreState>,
}

impl fmt::Debug for BoundedTraceSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        formatter
            .debug_struct("BoundedTraceSink")
            .field("policy", &self.policy)
            .field("active_run_count", &state.active_runs.len())
            .field("active_compilation_count", &state.active_compilations.len())
            .field("completed_bundle_count", &state.completed.len())
            .field("estimated_bytes", &state.estimated_bytes)
            .finish()
    }
}

impl BoundedTraceSink {
    pub fn new(policy: TraceRetentionPolicy) -> Result<Self, TraceRetentionError> {
        Self::with_clock(policy, Arc::new(SYSTEM_TRACE_CLOCK))
    }

    pub fn with_clock(
        policy: TraceRetentionPolicy,
        clock: Arc<dyn TraceClock>,
    ) -> Result<Self, TraceRetentionError> {
        policy.validate()?;
        Ok(Self {
            policy,
            clock,
            state: Mutex::new(TraceStoreState::default()),
        })
    }

    pub fn bundles(&self) -> Vec<TraceBundle> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .completed
            .iter()
            .cloned()
            .collect()
    }

    pub fn bundles_for_graph(&self, graph_path: &GraphResourcePath) -> Vec<TraceBundle> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .completed
            .iter()
            .filter(|bundle| bundle.is_associated_with_graph(graph_path))
            .cloned()
            .collect()
    }

    pub fn run_bundle(&self, run_id: RunId) -> Option<RunTraceBundle> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .completed
            .iter()
            .find_map(|bundle| match bundle {
                TraceBundle::Run(bundle) if bundle.run_id == run_id => Some(bundle.clone()),
                TraceBundle::Compilation(_) | TraceBundle::Run(_) => None,
            })
    }

    pub fn associate_run_incident(&self, run_id: RunId, incident_id: impl Into<Box<str>>) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(index) = state.completed.iter().position(
            |bundle| matches!(bundle, TraceBundle::Run(bundle) if bundle.run_id == run_id),
        ) else {
            return false;
        };
        let previous_bytes = state.completed[index].metadata().estimated_bytes;
        let new_bytes = {
            let TraceBundle::Run(bundle) = &mut state.completed[index] else {
                unreachable!("run bundle position was matched above")
            };
            bundle.incident_id = Some(incident_id.into());
            bundle.metadata.estimated_bytes = estimate_run_bundle(bundle);
            bundle.metadata.estimated_bytes
        };
        state.estimated_bytes = state
            .estimated_bytes
            .saturating_sub(previous_bytes)
            .saturating_add(new_bytes);
        enforce_retention(&mut state, self.policy);
        true
    }

    #[cfg(test)]
    pub(super) fn active_run_stats(&self, run_id: RunId) -> Option<(usize, u64, u64)> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active_runs
            .get(&run_id)
            .map(|active| {
                (
                    active.spans.len(),
                    active.estimated_span_bytes,
                    active.dropped_span_count,
                )
            })
    }

    #[cfg(test)]
    pub(super) fn active_bundle_counts(&self) -> (usize, usize) {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        (state.active_runs.len(), state.active_compilations.len())
    }

    fn record_completed_span(&self, span: TraceSpan) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        match span.run_id {
            Some(run_id) => {
                let completes_run = {
                    let Some(active) = state.active_runs.get_mut(&run_id) else {
                        return;
                    };
                    let completes_run =
                        active.root_span_id == span.span_id && is_top_level_run_root(&span);
                    active.record(span, self.policy);
                    completes_run
                };
                if !completes_run {
                    return;
                }
                let active = state
                    .active_runs
                    .remove(&run_id)
                    .expect("the completed run lifecycle was present above");
                let (spans, dropped_span_count) = active.into_parts();
                let span_count = spans.len();
                if let Some(bundle) = build_run_bundle(
                    run_id,
                    spans,
                    dropped_span_count,
                    self.policy.max_estimated_bytes as u64,
                ) {
                    retain_bundle(&mut state, TraceBundle::Run(bundle), self.policy);
                } else {
                    tracing::error!(
                        target: "yssbi::execution_trace",
                        trace_bundle_kind = "run",
                        run_id = run_id.get(),
                        span_count,
                        dropped_span_count,
                        "Execution trace bundle failed commit validation"
                    );
                }
            }
            None => {
                let completes_compilation =
                    span.kind == SpanKind::Snapshot && span.parent_span_id.is_none();
                let root_span_id = if completes_compilation {
                    span.span_id
                } else {
                    let Some(parent_span_id) = span.parent_span_id else {
                        return;
                    };
                    parent_span_id
                };
                let compile_id = span.correlation.compile_id;
                {
                    let Some(active) = state.active_compilations.get_mut(&root_span_id) else {
                        return;
                    };
                    active.record(span, self.policy);
                }
                if !completes_compilation {
                    return;
                }
                let active = state
                    .active_compilations
                    .remove(&root_span_id)
                    .expect("the completed compilation lifecycle was present above");
                let (spans, dropped_span_count) = active.into_parts();
                let span_count = spans.len();
                if let Some(bundle) = build_compilation_bundle(
                    compile_id,
                    spans,
                    dropped_span_count,
                    self.policy.max_estimated_bytes as u64,
                ) {
                    retain_bundle(&mut state, TraceBundle::Compilation(bundle), self.policy);
                } else {
                    tracing::error!(
                        target: "yssbi::execution_trace",
                        trace_bundle_kind = "compilation",
                        compile_id = compile_id.get(),
                        root_span_id = root_span_id.get(),
                        span_count,
                        dropped_span_count,
                        "Execution trace bundle failed commit validation"
                    );
                }
            }
        }
    }
}

impl Default for BoundedTraceSink {
    fn default() -> Self {
        Self::new(TraceRetentionPolicy::default())
            .expect("default trace retention limits are non-zero")
    }
}

impl TraceSink for BoundedTraceSink {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        let run_root = if spec.kind == SpanKind::Run
            && spec.parent_span_id.is_none()
            && spec.correlation.parent_call.is_none()
        {
            spec.run_id
        } else {
            None
        };
        let compilation_root = spec.run_id.is_none()
            && spec.kind == SpanKind::Snapshot
            && spec.parent_span_id.is_none();
        let guard = SpanGuard::new(self, spec, self.clock.as_ref());
        if run_root.is_some() || compilation_root {
            let root_span_id = guard.span_id();
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(run_id) = run_root {
                state
                    .active_runs
                    .entry(run_id)
                    .or_insert_with(|| ActiveTrace::new(root_span_id));
            } else {
                state
                    .active_compilations
                    .insert(root_span_id, ActiveTrace::new(root_span_id));
            }
        }
        guard
    }

    fn complete_span(&self, span: TraceSpan) {
        self.record_completed_span(span);
    }
}

fn retain_bundle(state: &mut TraceStoreState, bundle: TraceBundle, policy: TraceRetentionPolicy) {
    state.estimated_bytes = state
        .estimated_bytes
        .saturating_add(bundle.metadata().estimated_bytes);
    if matches!(bundle, TraceBundle::Run(_)) {
        state.completed_run_count = state.completed_run_count.saturating_add(1);
    }
    state.completed.push_back(bundle);
    enforce_retention(state, policy);
}

fn enforce_retention(state: &mut TraceStoreState, policy: TraceRetentionPolicy) {
    while state.completed_run_count > policy.max_completed_runs {
        let Some(index) = state
            .completed
            .iter()
            .position(|bundle| matches!(bundle, TraceBundle::Run(_)))
        else {
            break;
        };
        if let Some(bundle) = state.completed.remove(index) {
            remove_retained_bundle(state, &bundle);
        }
    }
    while state.estimated_bytes > policy.max_estimated_bytes as u64 && state.completed.len() > 1 {
        if let Some(bundle) = state.completed.pop_front() {
            remove_retained_bundle(state, &bundle);
        }
    }
}

fn remove_retained_bundle(state: &mut TraceStoreState, bundle: &TraceBundle) {
    state.estimated_bytes = state
        .estimated_bytes
        .saturating_sub(bundle.metadata().estimated_bytes);
    if matches!(bundle, TraceBundle::Run(_)) {
        state.completed_run_count = state.completed_run_count.saturating_sub(1);
    }
}
