use crate::execution::plan::{CompiledExecutionPackage, PlanCompilationBasis};
use crate::graph::analysis::{GraphAnalysis, GraphAnalysisInput, analyze};
use crate::graph::error::GraphCompileError;
use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use crate::graph::settings::GraphCompileSettings;
use crate::graph_document::GraphDocument;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphDiagnostic {
    pub code: Box<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompilationReport {
    pub analysis: GraphAnalysis,
    pub diagnostics: Box<[GraphDiagnostic]>,
    pub executable: Option<CompiledExecutionPackage>,
    pub basis: PlanCompilationBasis,
}

pub fn compile(
    document: &GraphDocument,
    catalog: &ResourceCatalogSnapshot,
    settings: &GraphCompileSettings,
    basis: PlanCompilationBasis,
) -> Result<CompilationReport, GraphCompileError> {
    let analysis = analyze(GraphAnalysisInput {
        document,
        catalog,
        settings,
        basis: &basis,
    });
    Ok(CompilationReport {
        analysis,
        diagnostics: Box::new([]),
        executable: None,
        basis,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::plan::{
        PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint,
    };
    use std::collections::BTreeMap;

    #[test]
    fn compiler_reports_neutral_analysis_without_execution_or_project_state() {
        let catalog = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            crate::graph::resource_catalog::ResourceCatalogFingerprint::from_bytes([1; 32]),
        );
        let settings = GraphCompileSettings {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-9,
        };
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("project".into()),
            PlanGraphRevision::from_existing(2),
            PlanRegistryFingerprint::from_bytes([2; 32]),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let report = compile(&GraphDocument::default(), &catalog, &settings, basis).unwrap();
        assert!(report.executable.is_none());
        assert!(report.analysis.nodes().is_empty());
    }
}
