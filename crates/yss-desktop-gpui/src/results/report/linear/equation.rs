use super::super::display;
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Selectable, Sizable,
    button::{Button, ButtonVariants},
    table::DataTable,
};
use gpui_kit::{Div, div, prelude::*, px};
use yss_application::graph::results::report::coefficients::RegressionCoefficient;

pub(super) struct EquationData {
    symbolic: String,
    expanded: String,
    pub mappings: Vec<Vec<String>>,
}

impl EquationData {
    pub fn new(report: &LinearRegressionReportProjection, rows: &[RegressionCoefficient]) -> Self {
        let mut terms = Vec::with_capacity(rows.len());
        let mut expanded = format!("{:?} = ", report.endog_name);
        let mut mappings = vec![vec!["y".into(), report.endog_name.clone(), "—".into()]];
        for (index, row) in rows.iter().enumerate() {
            // Parameter order and the model flag own intercept identity, never the display label.
            let intercept = report.constant && index == 0;
            let index_text = subscript(index + usize::from(!report.constant));
            let beta = format!("β{index_text}");
            let variable = format!("x{index_text}");
            terms.push(if intercept {
                beta.clone()
            } else {
                format!("{beta} {variable}")
            });
            mappings.push(vec![
                if intercept { beta } else { variable },
                row.variable.clone(),
                display::number(row.coef, 4),
            ]);
            if index > 0 {
                expanded.push_str(if row.coef.is_sign_negative() {
                    " − "
                } else {
                    " + "
                });
            } else if row.coef.is_sign_negative() {
                expanded.push('−');
            }
            expanded.push_str(&display::number(row.coef.abs(), 4));
            if !intercept {
                expanded.push_str(&format!(" · {:?}", row.variable));
            }
        }
        expanded.push_str(" + ε");
        Self {
            symbolic: format!("y = {} + ε", terms.join(" + ")),
            expanded,
            mappings,
        }
    }
}

fn subscript(index: usize) -> String {
    const DIGITS: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];
    index
        .to_string()
        .bytes()
        .map(|digit| DIGITS[(digit - b'0') as usize])
        .collect()
}

impl LinearReport {
    pub(super) fn render_equation(&self, cx: &mut Context<Self>) -> Div {
        let Some(equation) = &self.equation else {
            if self
                .coefficients
                .as_ref()
                .is_some_and(|page| page.total_count == 0)
            {
                return div()
                    .text_sm()
                    .child(crate::text::translate("native.reports.emptyCoefficients"));
            }
            return div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(crate::text::format(
                    "native.reports.equationLimit",
                    &[("count", COEFFICIENT_PAGE_ROWS.to_string())],
                ));
        };
        let formula = if self.symbolic {
            &equation.symbolic
        } else {
            &equation.expanded
        };
        let copied = formula.clone();
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .gap_2()
                    .children(
                        [
                            (true, "native.reports.symbolic"),
                            (false, "native.reports.expanded"),
                        ]
                        .map(|(symbolic, key)| {
                            Button::new(("equation-mode", usize::from(symbolic)))
                                .small()
                                .ghost()
                                .selected(self.symbolic == symbolic)
                                .label(crate::text::translate(key))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.symbolic = symbolic;
                                    cx.notify();
                                }))
                        }),
                    )
                    .child(
                        Button::new("copy-linear-equation")
                            .small()
                            .ghost()
                            .icon(IconName::Copy)
                            .tooltip(crate::text::translate("menubar.copy"))
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                    copied.clone(),
                                ))
                            }),
                    ),
            )
            .child(
                div()
                    .id("linear-equation-scroll")
                    .min_w_0()
                    .overflow_x_scroll()
                    .p_3()
                    .child(
                        div()
                            .whitespace_nowrap()
                            .text_lg()
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(formula.clone()),
                    ),
            )
            .when(self.symbolic, |body| {
                body.when_some(self.mapping.as_ref(), |body, table| {
                    body.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::text::translate("native.reports.mapping")),
                    )
                    .child(
                        div()
                            .h(px((equation.mappings.len().min(6) as f32 + 1.) * 32. + 20.))
                            .child(DataTable::new(table).small().stripe(true)),
                    )
                })
            })
    }
}
