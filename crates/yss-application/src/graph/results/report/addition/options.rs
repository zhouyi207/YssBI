use std::collections::BTreeSet;
use yss_graph_document::ParameterValues;

use super::ReportAdditionError;
use crate::graph::results::report::LinearSummaryOptions;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinearSummaryContent {
    ModelSummary,
    CoefficientTable,
    CoefficientChart,
    Equation,
    Anova,
    Diagnostics,
    ResidualPlot,
    Observations,
    AcfPacf,
    SerialTests,
    HypothesisTest,
}

impl LinearSummaryContent {
    pub const ALL: [Self; 11] = [
        Self::ModelSummary,
        Self::CoefficientTable,
        Self::CoefficientChart,
        Self::Equation,
        Self::Anova,
        Self::Diagnostics,
        Self::ResidualPlot,
        Self::Observations,
        Self::AcfPacf,
        Self::SerialTests,
        Self::HypothesisTest,
    ];

    pub fn included(self, options: &LinearSummaryOptions) -> bool {
        match self {
            Self::ModelSummary => options.model_summary,
            Self::CoefficientTable => options.coefficient_table,
            Self::CoefficientChart => options.coefficient_chart,
            Self::Equation => options.equation,
            Self::Anova => options.anova,
            Self::Diagnostics => options.diagnostics,
            Self::ResidualPlot => options.residual_plot,
            Self::Observations => options.observations,
            Self::AcfPacf => options.acf_pacf,
            Self::SerialTests => options.serial_tests,
            Self::HypothesisTest => options.hypothesis_test,
        }
    }

    fn parameter(self) -> &'static str {
        match self {
            Self::ModelSummary => "model_summary",
            Self::CoefficientTable => "coefficient_table",
            Self::CoefficientChart => "coefficient_chart",
            Self::Equation => "equation",
            Self::Anova => "anova",
            Self::Diagnostics => "diagnostics",
            Self::ResidualPlot => "residual_plot",
            Self::Observations => "observations",
            Self::AcfPacf => "acf_pacf",
            Self::SerialTests => "serial_tests",
            Self::HypothesisTest => "hypothesis_test",
        }
    }
}

/// Only selected contents and their associated inputs participate in the edit.
#[derive(Clone)]
pub struct LinearSummaryAddition {
    pub contents: BTreeSet<LinearSummaryContent>,
    pub acf_max_lag: usize,
    pub serial_lags: usize,
    pub bg_nomiss0: bool,
    pub hypothesis: String,
}

impl LinearSummaryAddition {
    pub fn is_valid(&self) -> bool {
        use LinearSummaryContent::*;
        !self.contents.is_empty()
            && (!self.contents.contains(&AcfPacf) || (1..=40).contains(&self.acf_max_lag))
            && (!self.contents.contains(&SerialTests) || (1..=40).contains(&self.serial_lags))
            && (!self.contents.contains(&HypothesisTest)
                || (!self.hypothesis.trim().is_empty() && self.hypothesis.len() <= 4096))
    }

    pub(super) fn parameters(&self) -> Result<ParameterValues, ReportAdditionError> {
        if !self.is_valid() {
            return Err(ReportAdditionError::InvalidSelection);
        }
        let mut values = ParameterValues::new();
        let mut insert = |key: &str, value| {
            values.insert(key.parse().expect("static parameter key"), value);
        };
        for content in &self.contents {
            insert(content.parameter(), true.into());
        }
        if self.contents.contains(&LinearSummaryContent::AcfPacf) {
            insert("acf_max_lag", self.acf_max_lag.into());
        }
        if self.contents.contains(&LinearSummaryContent::SerialTests) {
            insert("serial_lags", self.serial_lags.into());
            insert("bg_nomiss0", self.bg_nomiss0.into());
        }
        if self
            .contents
            .contains(&LinearSummaryContent::HypothesisTest)
        {
            insert("hypothesis", self.hypothesis.trim().into());
        }
        Ok(values)
    }
}
