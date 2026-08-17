use super::{
    CompilationTraceBundle, CompileId, RunId, RunTraceBundle, SpanId, SpanKind, SpanOutcome,
    TraceBundleMetadata, TraceProvenanceScope, TraceSpan,
};
use std::collections::{HashMap, HashSet};
use std::mem::size_of;

pub(super) fn is_top_level_run_root(span: &TraceSpan) -> bool {
    span.kind == SpanKind::Run
        && span.parent_span_id.is_none()
        && span.correlation.parent_call.is_none()
}

pub(super) fn build_run_bundle(
    run_id: RunId,
    mut spans: Vec<TraceSpan>,
    dropped_span_count: u64,
    byte_limit: u64,
) -> Option<RunTraceBundle> {
    sort_spans(&mut spans);
    if !valid_run_bundle(run_id, &spans) {
        return None;
    }
    let root = spans
        .iter()
        .find(|span| is_top_level_run_root(span))
        .expect("validated run bundles have one top-level root");
    let mut bundle = RunTraceBundle {
        run_id,
        compile_id: root.correlation.compile_id,
        graph_path: root.correlation.graph_path.clone(),
        selection_digest: root.correlation.selection_digest.clone(),
        incident_id: None,
        metadata: TraceBundleMetadata {
            provenance_scopes: collect_provenance_scopes(&spans),
            truncated: dropped_span_count > 0,
            dropped_span_count,
            estimated_bytes: 0,
        },
        spans: spans.into_boxed_slice(),
    };
    bundle.metadata.estimated_bytes = estimate_run_bundle(&bundle);
    truncate_run_bundle(&mut bundle, byte_limit);
    Some(bundle)
}

pub(super) fn build_compilation_bundle(
    compile_id: CompileId,
    mut spans: Vec<TraceSpan>,
    dropped_span_count: u64,
    byte_limit: u64,
) -> Option<CompilationTraceBundle> {
    sort_spans(&mut spans);
    if !valid_compilation_bundle(compile_id, &spans) {
        return None;
    }
    let root = spans
        .iter()
        .find(|span| span.kind == SpanKind::Snapshot && span.parent_span_id.is_none())
        .expect("validated compilation bundles have one snapshot root");
    let mut bundle = CompilationTraceBundle {
        compile_id,
        graph_path: root.correlation.graph_path.clone(),
        metadata: TraceBundleMetadata {
            provenance_scopes: collect_provenance_scopes(&spans),
            truncated: dropped_span_count > 0,
            dropped_span_count,
            estimated_bytes: 0,
        },
        spans: spans.into_boxed_slice(),
    };
    bundle.metadata.estimated_bytes = estimate_compilation_bundle(&bundle);
    truncate_compilation_bundle(&mut bundle, byte_limit);
    Some(bundle)
}

fn sort_spans(spans: &mut [TraceSpan]) {
    spans.sort_by_key(|span| (span.started_at, span.span_id));
}

fn valid_run_bundle(run_id: RunId, spans: &[TraceSpan]) -> bool {
    spans
        .iter()
        .filter(|span| is_top_level_run_root(span))
        .count()
        == 1
        && spans.iter().all(|span| span.run_id == Some(run_id))
        && valid_span_hierarchy(spans)
}

fn valid_compilation_bundle(compile_id: CompileId, spans: &[TraceSpan]) -> bool {
    spans
        .iter()
        .filter(|span| span.kind == SpanKind::Snapshot && span.parent_span_id.is_none())
        .count()
        == 1
        && spans
            .iter()
            .all(|span| span.run_id.is_none() && span.correlation.compile_id == compile_id)
        && valid_span_hierarchy(spans)
}

fn valid_span_hierarchy(spans: &[TraceSpan]) -> bool {
    let mut indices = HashMap::with_capacity(spans.len());
    for (index, span) in spans.iter().enumerate() {
        if indices.insert(span.span_id, index).is_some() {
            return false;
        }
    }
    let mut parents = Vec::with_capacity(spans.len());
    for (index, span) in spans.iter().enumerate() {
        let parent = match span.parent_span_id {
            None => None,
            Some(parent_id) => match indices.get(&parent_id).copied() {
                Some(parent) if parent != index => Some(parent),
                _ => return false,
            },
        };
        if !compatible_parent(span, parent.map(|parent| &spans[parent])) {
            return false;
        }
        parents.push(parent);
    }

    let mut colors = vec![0_u8; spans.len()];
    for start in 0..spans.len() {
        if colors[start] != 0 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(index) = current.filter(|index| colors[*index] == 0) {
            colors[index] = 1;
            path.push(index);
            current = parents[index];
        }
        if current.is_some_and(|index| colors[index] == 1) {
            return false;
        }
        for index in path {
            colors[index] = 2;
        }
    }
    true
}

fn compatible_parent(span: &TraceSpan, parent: Option<&TraceSpan>) -> bool {
    if !has_valid_kind_semantics(span) {
        return false;
    }
    if parent.is_some_and(|parent| !same_lineage(span, parent)) {
        return false;
    }
    if is_runtime_kind(span.kind) && parent.is_some_and(|parent| !interval_contains(parent, span)) {
        return false;
    }
    match span.kind {
        SpanKind::Snapshot => parent.is_none(),
        SpanKind::Analysis | SpanKind::Lowering => {
            parent.is_some_and(|parent| parent.kind == SpanKind::Snapshot)
        }
        SpanKind::Run => parent.is_none_or(|parent| parent.kind == SpanKind::Run),
        SpanKind::ResourceAcquire
        | SpanKind::ResultPublication
        | SpanKind::Cleanup
        | SpanKind::OperationAttempt => parent.is_some_and(|parent| parent.kind == SpanKind::Run),
        SpanKind::AdapterIo => parent.is_some_and(|parent| {
            parent.kind == SpanKind::OperationAttempt
                && span.operation_id == parent.operation_id
                && span.activation_id == parent.activation_id
                && span.attempt_id == parent.attempt_id
        }),
    }
}

fn has_valid_kind_semantics(span: &TraceSpan) -> bool {
    if span.finished_at < span.started_at || span.run_id != span.correlation.run_id {
        return false;
    }
    match span.kind {
        SpanKind::Snapshot | SpanKind::Analysis | SpanKind::Lowering => {
            span.run_id.is_none()
                && has_no_operation_identity(span)
                && is_general_outcome(&span.outcome)
        }
        SpanKind::Run => {
            span.run_id.is_some()
                && has_no_operation_identity(span)
                && is_general_outcome(&span.outcome)
        }
        SpanKind::ResourceAcquire | SpanKind::ResultPublication => {
            span.run_id.is_some()
                && has_no_operation_identity(span)
                && is_phase_outcome(&span.outcome)
        }
        SpanKind::Cleanup => {
            span.run_id.is_some()
                && has_no_operation_identity(span)
                && matches!(
                    span.outcome,
                    SpanOutcome::NotReached
                        | SpanOutcome::Cleanup { .. }
                        | SpanOutcome::InternalAborted
                )
        }
        SpanKind::OperationAttempt => {
            span.run_id.is_some()
                && has_operation_identity(span)
                && (is_general_outcome(&span.outcome) || span.outcome == SpanOutcome::Retry)
        }
        SpanKind::AdapterIo => {
            span.run_id.is_some()
                && has_operation_identity(span)
                && is_general_outcome(&span.outcome)
        }
    }
}

fn has_operation_identity(span: &TraceSpan) -> bool {
    span.operation_id.is_some() && span.activation_id.is_some() && span.attempt_id.is_some()
}

fn is_general_outcome(outcome: &SpanOutcome) -> bool {
    matches!(
        outcome,
        SpanOutcome::Success
            | SpanOutcome::Error
            | SpanOutcome::Cancellation
            | SpanOutcome::Timeout
            | SpanOutcome::InternalAborted
    )
}

fn is_phase_outcome(outcome: &SpanOutcome) -> bool {
    is_general_outcome(outcome) || *outcome == SpanOutcome::NotReached
}

fn is_runtime_kind(kind: SpanKind) -> bool {
    !matches!(
        kind,
        SpanKind::Snapshot | SpanKind::Analysis | SpanKind::Lowering
    )
}

fn interval_contains(parent: &TraceSpan, child: &TraceSpan) -> bool {
    child.started_at >= parent.started_at && child.finished_at <= parent.finished_at
}

fn has_no_operation_identity(span: &TraceSpan) -> bool {
    span.operation_id.is_none() && span.activation_id.is_none() && span.attempt_id.is_none()
}

fn same_lineage(span: &TraceSpan, parent: &TraceSpan) -> bool {
    span.run_id == parent.run_id
        && span.correlation.project_session_id == parent.correlation.project_session_id
        && span.correlation.graph_path == parent.correlation.graph_path
        && span.correlation.graph_revision == parent.correlation.graph_revision
        && span.correlation.registry_fingerprint == parent.correlation.registry_fingerprint
        && span.correlation.compile_id == parent.correlation.compile_id
}

fn collect_provenance_scopes(spans: &[TraceSpan]) -> Box<[TraceProvenanceScope]> {
    let mut scopes = Vec::new();
    for span in spans {
        let scope = TraceProvenanceScope::from(&span.correlation);
        if !scopes.contains(&scope) {
            scopes.push(scope);
        }
    }
    scopes.into_boxed_slice()
}

fn truncate_run_bundle(bundle: &mut RunTraceBundle, byte_limit: u64) {
    if bundle.metadata.estimated_bytes <= byte_limit {
        return;
    }
    let root_id = bundle
        .spans
        .iter()
        .find(|span| is_top_level_run_root(span))
        .expect("validated run bundle root")
        .span_id;
    let original = std::mem::take(&mut bundle.spans).into_vec();
    let retained =
        retain_structural_subset(&original, root_id, byte_limit, estimate_run_base(bundle));
    let newly_dropped = original.len().saturating_sub(retained.len()) as u64;
    bundle.metadata.dropped_span_count = bundle
        .metadata
        .dropped_span_count
        .saturating_add(newly_dropped);
    bundle.metadata.truncated = bundle.metadata.dropped_span_count > 0;
    bundle.spans = retained.into_boxed_slice();
    bundle.metadata.estimated_bytes = estimate_run_bundle(bundle);
}

fn truncate_compilation_bundle(bundle: &mut CompilationTraceBundle, byte_limit: u64) {
    if bundle.metadata.estimated_bytes <= byte_limit {
        return;
    }
    let root_id = bundle
        .spans
        .iter()
        .find(|span| span.kind == SpanKind::Snapshot && span.parent_span_id.is_none())
        .expect("validated compilation bundle root")
        .span_id;
    let original = std::mem::take(&mut bundle.spans).into_vec();
    let retained = retain_structural_subset(
        &original,
        root_id,
        byte_limit,
        estimate_compilation_base(bundle),
    );
    let newly_dropped = original.len().saturating_sub(retained.len()) as u64;
    bundle.metadata.dropped_span_count = bundle
        .metadata
        .dropped_span_count
        .saturating_add(newly_dropped);
    bundle.metadata.truncated = bundle.metadata.dropped_span_count > 0;
    bundle.spans = retained.into_boxed_slice();
    bundle.metadata.estimated_bytes = estimate_compilation_bundle(bundle);
}

fn retain_structural_subset(
    spans: &[TraceSpan],
    primary_root: SpanId,
    byte_limit: u64,
    base_bytes: u64,
) -> Vec<TraceSpan> {
    let indices = spans
        .iter()
        .enumerate()
        .map(|(index, span)| (span.span_id, index))
        .collect::<HashMap<_, _>>();
    let mut depths = vec![0_usize; spans.len()];
    for index in 0..spans.len() {
        let mut depth = 0_usize;
        let mut current = spans[index].parent_span_id;
        while let Some(parent) = current.and_then(|id| indices.get(&id).copied()) {
            depth = depth.saturating_add(1);
            current = spans[parent].parent_span_id;
        }
        depths[index] = depth;
    }
    let mut candidates = (0..spans.len()).collect::<Vec<_>>();
    candidates.sort_by_key(|index| {
        let span = &spans[*index];
        (
            span.span_id != primary_root,
            depths[*index],
            span.started_at,
            span.span_id,
        )
    });

    let mut retained_ids = HashSet::new();
    let mut retained_indices = HashSet::new();
    let mut estimated_bytes = base_bytes;
    for index in candidates {
        let span = &spans[index];
        if span.span_id != primary_root
            && span
                .parent_span_id
                .is_some_and(|parent| !retained_ids.contains(&parent))
        {
            continue;
        }
        let span_bytes = estimate_span(span);
        if span.span_id != primary_root && estimated_bytes.saturating_add(span_bytes) > byte_limit {
            continue;
        }
        estimated_bytes = estimated_bytes.saturating_add(span_bytes);
        retained_ids.insert(span.span_id);
        retained_indices.insert(index);
    }
    spans
        .iter()
        .enumerate()
        .filter(|(index, _)| retained_indices.contains(index))
        .map(|(_, span)| span.clone())
        .collect()
}

pub(super) fn estimate_run_bundle(bundle: &RunTraceBundle) -> u64 {
    estimate_run_base(bundle).saturating_add(bundle.spans.iter().map(estimate_span).sum::<u64>())
}

fn estimate_compilation_bundle(bundle: &CompilationTraceBundle) -> u64 {
    estimate_compilation_base(bundle)
        .saturating_add(bundle.spans.iter().map(estimate_span).sum::<u64>())
}

fn estimate_run_base(bundle: &RunTraceBundle) -> u64 {
    (size_of::<RunTraceBundle>() as u64)
        .saturating_add(bundle.graph_path.0.len() as u64)
        .saturating_add(
            bundle
                .selection_digest
                .as_deref()
                .map_or(0, |value| value.len() as u64),
        )
        .saturating_add(
            bundle
                .incident_id
                .as_deref()
                .map_or(0, |value| value.len() as u64),
        )
        .saturating_add(
            bundle
                .metadata
                .provenance_scopes
                .iter()
                .map(estimate_scope)
                .sum::<u64>(),
        )
}

fn estimate_compilation_base(bundle: &CompilationTraceBundle) -> u64 {
    (size_of::<CompilationTraceBundle>() as u64)
        .saturating_add(bundle.graph_path.0.len() as u64)
        .saturating_add(
            bundle
                .metadata
                .provenance_scopes
                .iter()
                .map(estimate_scope)
                .sum::<u64>(),
        )
}

fn estimate_scope(scope: &TraceProvenanceScope) -> u64 {
    (size_of::<TraceProvenanceScope>() as u64)
        .saturating_add(scope.project_session_id.as_str().len() as u64)
        .saturating_add(scope.graph_path.0.len() as u64)
        .saturating_add(
            scope
                .resource_versions
                .iter()
                .map(|(key, version)| (key.as_str().len() + version.as_str().len()) as u64)
                .sum::<u64>(),
        )
}

pub(super) fn estimate_span(span: &TraceSpan) -> u64 {
    (size_of::<TraceSpan>() as u64)
        .saturating_add(span.correlation.project_session_id.as_str().len() as u64)
        .saturating_add(span.correlation.graph_path.0.len() as u64)
        .saturating_add(
            span.operation_id
                .as_ref()
                .map_or(0, |value| value.as_str().len() as u64),
        )
        .saturating_add(
            span.correlation
                .selection_digest
                .as_deref()
                .map_or(0, |value| value.len() as u64),
        )
        .saturating_add(
            span.correlation
                .node_type_id
                .as_ref()
                .map_or(0, |value| value.as_str().len() as u64),
        )
        .saturating_add(
            span.correlation
                .resource_versions
                .iter()
                .map(|(key, version)| (key.as_str().len() + version.as_str().len()) as u64)
                .sum::<u64>(),
        )
}
