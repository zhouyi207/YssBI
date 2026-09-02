use std::collections::BTreeMap;

use yss_automation_contract::{
    DiagnosticRequirement, MethodVersion, StatisticalMethodCard, StatisticalMethodId,
    StatisticalPlan, StatisticalPlanError, StudyDesignKind, VariableRequirement, VariableRole,
};

#[derive(Clone, Debug)]
pub struct MethodRegistry {
    cards: BTreeMap<StatisticalMethodId, StatisticalMethodCard>,
}

impl MethodRegistry {
    pub fn builtins() -> Result<Self, StatisticalPlannerError> {
        let cards = [ols_card()?, descriptive_card()?]
            .into_iter()
            .map(|card| (card.id.clone(), card))
            .collect();
        Ok(Self { cards })
    }

    pub fn card(&self, id: &StatisticalMethodId) -> Option<&StatisticalMethodCard> {
        self.cards.get(id)
    }
}

pub struct StatisticalPlanner;

impl StatisticalPlanner {
    pub fn validate(
        plan: &StatisticalPlan,
        methods: &MethodRegistry,
    ) -> Result<(), StatisticalPlannerError> {
        plan.validate()?;
        let mut applicable = 0usize;
        for method_id in &plan.candidate_methods {
            let card = methods
                .card(method_id)
                .ok_or(StatisticalPlannerError::UnknownMethod)?;
            if method_is_applicable(plan, card) {
                applicable += 1;
            }
        }
        if applicable == 0 {
            return Err(StatisticalPlannerError::NoApplicableMethod);
        }
        Ok(())
    }
}

fn method_is_applicable(plan: &StatisticalPlan, card: &StatisticalMethodCard) -> bool {
    card.supported_designs.contains(&plan.study_design.kind)
        && card.variable_requirements.iter().all(|requirement| {
            let count = plan
                .variable_roles
                .iter()
                .filter(|assignment| assignment.role == requirement.role)
                .count();
            count >= usize::from(requirement.minimum_count)
                && requirement
                    .maximum_count
                    .is_none_or(|maximum| count <= usize::from(maximum))
        })
        && card
            .diagnostics
            .iter()
            .all(|diagnostic| plan.required_diagnostics.contains(diagnostic))
}

fn ols_card() -> Result<StatisticalMethodCard, StatisticalPlannerError> {
    Ok(StatisticalMethodCard {
        id: StatisticalMethodId::try_new("yssbi.statistics.ols")?,
        version: MethodVersion::try_new("1.0.0")?,
        supported_designs: vec![
            StudyDesignKind::CrossSectional,
            StudyDesignKind::Longitudinal,
            StudyDesignKind::Observational,
        ],
        variable_requirements: vec![
            VariableRequirement {
                role: VariableRole::Outcome,
                minimum_count: 1,
                maximum_count: Some(1),
            },
            VariableRequirement {
                role: VariableRole::Predictor,
                minimum_count: 1,
                maximum_count: None,
            },
        ],
        assumptions: vec![
            "linearity".to_owned(),
            "independent errors".to_owned(),
            "well-behaved residual variance".to_owned(),
        ],
        diagnostics: vec![
            DiagnosticRequirement::MeasurementScale,
            DiagnosticRequirement::Missingness,
            DiagnosticRequirement::Outliers,
            DiagnosticRequirement::ModelAssumptions,
            DiagnosticRequirement::InfluentialObservations,
            DiagnosticRequirement::MultipleTesting,
        ],
        alternatives: Vec::new(),
        reporting_requirements: vec![
            "effect estimates and confidence intervals".to_owned(),
            "diagnostic findings".to_owned(),
            "limitations".to_owned(),
        ],
    })
}

fn descriptive_card() -> Result<StatisticalMethodCard, StatisticalPlannerError> {
    Ok(StatisticalMethodCard {
        id: StatisticalMethodId::try_new("yssbi.statistics.descriptive")?,
        version: MethodVersion::try_new("1.0.0")?,
        supported_designs: vec![
            StudyDesignKind::CrossSectional,
            StudyDesignKind::Longitudinal,
            StudyDesignKind::TimeSeries,
            StudyDesignKind::RandomizedExperiment,
            StudyDesignKind::QuasiExperimental,
            StudyDesignKind::Observational,
        ],
        variable_requirements: vec![VariableRequirement {
            role: VariableRole::Outcome,
            minimum_count: 1,
            maximum_count: None,
        }],
        assumptions: Vec::new(),
        diagnostics: vec![
            DiagnosticRequirement::MeasurementScale,
            DiagnosticRequirement::Missingness,
            DiagnosticRequirement::Outliers,
            DiagnosticRequirement::MultipleTesting,
        ],
        alternatives: Vec::new(),
        reporting_requirements: vec![
            "sample definition".to_owned(),
            "distribution summaries".to_owned(),
            "limitations".to_owned(),
        ],
    })
}

#[derive(Debug, thiserror::Error)]
pub enum StatisticalPlannerError {
    #[error("statistical plan failed quality validation")]
    InvalidPlan(#[from] StatisticalPlanError),
    #[error("statistical method identity is invalid")]
    Identity(#[from] yss_automation_contract::AutomationIdentityError),
    #[error("statistical plan references an unknown method")]
    UnknownMethod,
    #[error("no candidate statistical method is applicable")]
    NoApplicableMethod,
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_automation_contract::{
        AnalysisMode, DiagnosticRequirement, Estimand, ReportingContract, RobustnessCheck,
        RobustnessCheckKind, StatisticalPlan, StudyDesign, VariableRoleAssignment, WorkflowId,
    };

    #[test]
    fn confirmatory_ols_requires_complete_quality_gates_and_variable_roles() {
        let mut plan = StatisticalPlan {
            research_question: "What is the adjusted association?".to_owned(),
            analysis_mode: AnalysisMode::Confirmatory,
            study_design: StudyDesign {
                kind: StudyDesignKind::CrossSectional,
                description: "A prespecified observational sample".to_owned(),
            },
            estimands: vec![Estimand {
                name: "adjusted mean difference".to_owned(),
                target_population: "observed study population".to_owned(),
                contrast: "one unit higher exposure".to_owned(),
            }],
            variable_roles: vec![
                VariableRoleAssignment {
                    resource_id: "database-1".to_owned(),
                    variable: "outcome".to_owned(),
                    role: VariableRole::Outcome,
                },
                VariableRoleAssignment {
                    resource_id: "database-1".to_owned(),
                    variable: "exposure".to_owned(),
                    role: VariableRole::Predictor,
                },
            ],
            candidate_methods: vec![StatisticalMethodId::try_new("yssbi.statistics.ols").unwrap()],
            selected_workflow: WorkflowId::try_new("ols_model_and_diagnostics").unwrap(),
            required_diagnostics: vec![
                DiagnosticRequirement::MeasurementScale,
                DiagnosticRequirement::Missingness,
                DiagnosticRequirement::Outliers,
                DiagnosticRequirement::ModelAssumptions,
                DiagnosticRequirement::InfluentialObservations,
                DiagnosticRequirement::MultipleTesting,
            ],
            robustness_checks: vec![RobustnessCheck {
                kind: RobustnessCheckKind::RobustStandardErrors,
                description: "Compare robust uncertainty estimates".to_owned(),
            }],
            reporting_contract: ReportingContract {
                require_effect_sizes: true,
                require_uncertainty: true,
                require_diagnostics: true,
                require_limitations: true,
                confidence_level: 0.95,
            },
        };
        let methods = MethodRegistry::builtins().unwrap();
        StatisticalPlanner::validate(&plan, &methods).unwrap();

        plan.variable_roles
            .retain(|assignment| assignment.role != VariableRole::Outcome);
        assert!(matches!(
            StatisticalPlanner::validate(&plan, &methods),
            Err(StatisticalPlannerError::NoApplicableMethod)
                | Err(StatisticalPlannerError::InvalidPlan(_))
        ));
    }
}
