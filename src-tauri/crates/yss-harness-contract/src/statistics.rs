use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{MethodVersion, StatisticalMethodId, WorkflowId};

#[derive(
    Clone, Copy, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisMode {
    Confirmatory,
    Exploratory,
    PostHoc,
}

#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyDesignKind {
    CrossSectional,
    Longitudinal,
    TimeSeries,
    RandomizedExperiment,
    QuasiExperimental,
    Observational,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyDesign {
    pub kind: StudyDesignKind,
    pub description: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Estimand {
    pub name: String,
    pub target_population: String,
    pub contrast: String,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum VariableRole {
    Outcome,
    Exposure,
    Predictor,
    Confounder,
    Mediator,
    Moderator,
    Group,
    Time,
    Identifier,
    Weight,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableRoleAssignment {
    pub resource_id: String,
    pub variable: String,
    pub role: VariableRole,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticRequirement {
    MeasurementScale,
    Missingness,
    Outliers,
    ModelAssumptions,
    InfluentialObservations,
    MultipleTesting,
    Convergence,
    Sensitivity,
}

#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustnessCheckKind {
    AlternativeSpecification,
    RobustStandardErrors,
    Subgroup,
    Placebo,
    Sensitivity,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RobustnessCheck {
    pub kind: RobustnessCheckKind,
    pub description: String,
}

#[derive(Clone, Copy, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportingContract {
    pub require_effect_sizes: bool,
    pub require_uncertainty: bool,
    pub require_diagnostics: bool,
    pub require_limitations: bool,
    pub confidence_level: f64,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatisticalPlan {
    pub research_question: String,
    pub analysis_mode: AnalysisMode,
    pub study_design: StudyDesign,
    pub estimands: Vec<Estimand>,
    pub variable_roles: Vec<VariableRoleAssignment>,
    pub candidate_methods: Vec<StatisticalMethodId>,
    pub selected_workflow: WorkflowId,
    pub required_diagnostics: Vec<DiagnosticRequirement>,
    pub robustness_checks: Vec<RobustnessCheck>,
    pub reporting_contract: ReportingContract,
}

impl StatisticalPlan {
    pub fn validate(&self) -> Result<(), StatisticalPlanError> {
        if !valid_text(&self.research_question, 4_096)
            || !valid_text(&self.study_design.description, 4_096)
            || self.candidate_methods.is_empty()
            || self.variable_roles.is_empty()
            || !self.reporting_contract.confidence_level.is_finite()
            || !(0.0..1.0).contains(&self.reporting_contract.confidence_level)
        {
            return Err(StatisticalPlanError::Incomplete);
        }
        if self.analysis_mode == AnalysisMode::Confirmatory && self.estimands.is_empty() {
            return Err(StatisticalPlanError::ConfirmatoryEstimandRequired);
        }
        if self.estimands.iter().any(|estimand| {
            !valid_text(&estimand.name, 256)
                || !valid_text(&estimand.target_population, 1_024)
                || !valid_text(&estimand.contrast, 1_024)
        }) || self.variable_roles.iter().any(|assignment| {
            !valid_text(&assignment.resource_id, 1_024) || !valid_text(&assignment.variable, 1_024)
        }) || self
            .robustness_checks
            .iter()
            .any(|check| !valid_text(&check.description, 2_048))
        {
            return Err(StatisticalPlanError::InvalidField);
        }
        if self.candidate_methods.iter().collect::<BTreeSet<_>>().len()
            != self.candidate_methods.len()
            || self
                .variable_roles
                .iter()
                .map(|assignment| {
                    (
                        &assignment.resource_id,
                        &assignment.variable,
                        assignment.role,
                    )
                })
                .collect::<BTreeSet<_>>()
                .len()
                != self.variable_roles.len()
        {
            return Err(StatisticalPlanError::DuplicateEntry);
        }
        let required = BTreeSet::from([
            DiagnosticRequirement::MeasurementScale,
            DiagnosticRequirement::Missingness,
            DiagnosticRequirement::Outliers,
            DiagnosticRequirement::ModelAssumptions,
            DiagnosticRequirement::MultipleTesting,
        ]);
        let actual = self
            .required_diagnostics
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if !required.is_subset(&actual)
            || !self.reporting_contract.require_effect_sizes
            || !self.reporting_contract.require_uncertainty
            || !self.reporting_contract.require_diagnostics
            || !self.reporting_contract.require_limitations
        {
            return Err(StatisticalPlanError::QualityGateMissing);
        }
        Ok(())
    }
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum StatisticalPlanError {
    #[error("statistical plan is incomplete")]
    Incomplete,
    #[error("confirmatory plan requires an estimand")]
    ConfirmatoryEstimandRequired,
    #[error("statistical plan contains an invalid field")]
    InvalidField,
    #[error("statistical plan contains a duplicate entry")]
    DuplicateEntry,
    #[error("statistical plan omits a required quality gate")]
    QualityGateMissing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableRequirement {
    pub role: VariableRole,
    pub minimum_count: u16,
    pub maximum_count: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatisticalMethodCard {
    pub id: StatisticalMethodId,
    pub version: MethodVersion,
    pub supported_designs: Vec<StudyDesignKind>,
    pub variable_requirements: Vec<VariableRequirement>,
    pub assumptions: Vec<String>,
    pub diagnostics: Vec<DiagnosticRequirement>,
    pub alternatives: Vec<StatisticalMethodId>,
    pub reporting_requirements: Vec<String>,
}

pub fn statistical_plan_schema() -> schemars::Schema {
    schemars::schema_for!(StatisticalPlan)
}
