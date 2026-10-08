//! One report owns its coefficient page; equation, table and bars read that same page.
mod coefficients;
mod equation;
mod render;

use gpui::{AppContext, Context, Entity};
use gpui_component::table::TableState;
use std::{collections::BTreeMap, sync::Arc};
use yss_application::graph::results::report::{
    LinearRegressionReportProjection,
    coefficients::LinearCoefficientPage,
    presentation::{self, DisplayData},
};

use super::{
    page::PageSource,
    section::{Section, Source, Title},
};
use crate::{results::table::ResultGrid, services::NativeServices};

const COEFFICIENT_PAGE_ROWS: usize = 200;

pub(super) struct LinearReport {
    services: Arc<NativeServices>,
    report: LinearRegressionReportProjection,
    presentation: BTreeMap<&'static str, DisplayData>,
    observations: Option<Entity<Section>>,
    coefficients: Option<Arc<LinearCoefficientPage>>,
    equation: Option<equation::EquationData>,
    table: Option<Entity<TableState<ResultGrid>>>,
    mapping: Option<Entity<TableState<ResultGrid>>>,
    anova: Option<Entity<TableState<ResultGrid>>>,
    symbolic: bool,
    initialized: bool,
    locale: String,
    requested_offset: usize,
    loading: bool,
    error: bool,
    task: Option<gpui::Task<()>>,
}

impl LinearReport {
    pub fn new(
        services: Arc<NativeServices>,
        report: LinearRegressionReportProjection,
        cx: &mut Context<Self>,
    ) -> Self {
        let observations = report.summary.observations.then(|| {
            cx.new(|_| {
                Section::new(
                    services.clone(),
                    report.reference,
                    Title::Key("reportSections.observations"),
                    Source::Page(PageSource::Observations),
                    false,
                )
            })
        });
        let presentation = presentation::data(&report.model, report.condition_number);
        Self {
            services,
            report,
            presentation,
            observations,
            coefficients: None,
            equation: None,
            table: None,
            mapping: None,
            anova: None,
            symbolic: true,
            initialized: false,
            locale: String::new(),
            requested_offset: 0,
            loading: false,
            error: false,
            task: None,
        }
    }

    fn has_coefficients(&self) -> bool {
        let selected = &self.report.summary;
        selected.equation || selected.coefficient_table || selected.coefficient_chart
    }

    fn pending_analyses(&self) -> bool {
        let selected = &self.report.summary;
        selected.diagnostics
            || selected.residual_plot
            || selected.acf_pacf
            || selected.serial_tests
            || selected.hypothesis_test
    }
}
