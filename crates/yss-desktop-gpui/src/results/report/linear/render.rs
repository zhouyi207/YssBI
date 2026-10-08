use super::super::display;
use super::*;
use gpui::{IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme, Sizable, StyledExt, table::DataTable};

impl Render for LinearReport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.initialized {
            self.initialized = true;
            self.refresh_tables(window, cx);
            if self.has_coefficients() {
                self.load(0, window, cx);
            }
        } else if self.locale.as_str() != &*gpui_component::locale() {
            // Rebuild localized table headings from accepted data, without another query.
            self.refresh_tables(window, cx);
        }
        let options = &self.report.summary;
        let mut body = div().w_full().min_w_0().flex().flex_col().gap_4().child(
            div()
                .text_lg()
                .font_semibold()
                .child(self.report.title.clone()),
        );
        if options.equation && self.coefficients.is_some() {
            body = body.child(display::section(
                "reportSections.equation",
                self.render_equation(cx),
                cx,
            ));
        }
        if options.model_summary
            && let Some(data) = self.presentation.get("summary")
        {
            body = body.child(display::section(
                "reportSections.modelSummary",
                display::metrics(data, cx),
                cx,
            ));
        }
        if let Some(table) = &self.anova {
            body = body.child(display::section(
                "reportSections.anova",
                div()
                    .h(px(148.))
                    .child(DataTable::new(table).small().stripe(true)),
                cx,
            ));
        }
        if let Some(page) = &self.coefficients {
            if let Some(table) = &self.table {
                let significant = page
                    .coefficients
                    .iter()
                    .filter(|row| row.is_significant)
                    .count();
                body = body.child(display::section(
                    "reportSections.coefficientTable",
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_xs().child(crate::text::format(
                            "native.reports.significantCount",
                            &[
                                ("count", significant.to_string()),
                                ("total", page.coefficients.len().to_string()),
                            ],
                        )))
                        .child(
                            div()
                                .h(px((page.coefficients.len().min(10) as f32 + 1.) * 32. + 20.))
                                .child(DataTable::new(table).small().stripe(true)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(crate::text::translate("native.reports.significance")),
                        ),
                    cx,
                ));
            }
            if options.coefficient_chart {
                body = body.child(display::section(
                    "reportSections.coefficientMagnitude",
                    self.bars(cx),
                    cx,
                ));
            }
        }
        if self.has_coefficients() {
            body = body.child(self.paging(cx));
        }
        if options.diagnostics
            && let Some(data) = self.presentation.get("conditionNumber")
        {
            body = body.child(display::section(
                "reportSections.diagnostics",
                display::metrics(data, cx),
                cx,
            ));
        }
        body.children(self.sections.iter().cloned()).when(
            !self.has_coefficients()
                && !options.model_summary
                && !options.anova
                && self.sections.is_empty(),
            |body| body.child(crate::text::translate("native.reports.emptySelection")),
        )
    }
}
