use super::super::display;
use super::*;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{App, Div, IntoElement, Window, div, prelude::*, px, relative};

impl LinearReport {
    pub(super) fn load(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = false;
        self.requested_offset = offset;
        let reference = self.report.reference;
        let task = self.services.run(move |services| {
            Ok(services.application.query_result_coefficients(
                reference,
                offset,
                COEFFICIENT_PAGE_ROWS,
            )?)
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                view.loading = false;
                match result {
                    Ok(page) => {
                        view.equation = (view.report.summary.equation
                            && page.offset == 0
                            && page.total_count > 0
                            && page.coefficients.len() == page.total_count)
                            .then(|| equation::EquationData::new(&view.report, &page.coefficients));
                        view.coefficients = Some(Arc::new(page));
                        view.refresh_tables(window, cx);
                    }
                    Err(_) => view.error = true,
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn refresh_tables(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.locale = gpui_kit::component::locale().to_string();
        if self.report.summary.anova {
            self.anova = self
                .presentation
                .get("anova")
                .and_then(display::table)
                .map(|grid| cx.new(|cx| TableState::new(grid, window, cx).sortable(false)));
        }
        if self.report.summary.coefficient_table {
            self.table = self.coefficients.as_ref().map(|page| {
                let names = [
                    "variable",
                    "coef",
                    "stdErr",
                    "t",
                    "p",
                    "lower",
                    "upper",
                    "significant",
                ]
                .map(|key| crate::text::translate(&format!("native.reports.columns.{key}")))
                .to_vec();
                let rows = page
                    .coefficients
                    .iter()
                    .map(|c| {
                        vec![
                            c.variable.clone(),
                            display::number(c.coef, 4),
                            display::number(c.std_err, 4),
                            display::number(c.t_value, 3),
                            format!("{} {}", display::number(c.p_value, 3), stars(c.p_value)),
                            display::number(c.ci_lower, 4),
                            display::number(c.ci_upper, 4),
                            crate::text::translate(if c.is_significant {
                                "native.reports.yes"
                            } else {
                                "native.reports.no"
                            }),
                        ]
                    })
                    .collect();
                cx.new(|cx| {
                    TableState::new(ResultGrid::formatted(names, rows), window, cx).sortable(false)
                })
            });
        }
        self.mapping = self.equation.as_ref().map(|equation| {
            let names = ["symbol", "variable", "coef"]
                .map(|key| crate::text::translate(&format!("native.reports.columns.{key}")))
                .to_vec();
            cx.new(|cx| {
                TableState::new(
                    ResultGrid::formatted(names, equation.mappings.clone()),
                    window,
                    cx,
                )
                .sortable(false)
            })
        });
    }

    pub(super) fn paging(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_wrap()
            .justify_end()
            .items_center()
            .gap_2()
            .text_xs()
            .when(self.loading, |bar| {
                bar.child(crate::text::translate("dataOperation.reading"))
            })
            .when(self.error, |bar| {
                bar.child(
                    div()
                        .text_color(cx.theme().danger)
                        .child(crate::text::translate("native.reports.pageFailed")),
                )
                .child(
                    Button::new("retry-coefficients")
                        .small()
                        .ghost()
                        .label(crate::text::translate("common.retry"))
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.load(view.requested_offset, window, cx)
                        })),
                )
            })
            .when_some(self.coefficients.as_ref(), |bar, page| {
                let offset = page.offset;
                let count = page.coefficients.len();
                bar.child(format!(
                    "{}–{} / {}",
                    if count == 0 { 0 } else { offset + 1 },
                    offset + count,
                    page.total_count
                ))
                .child(
                    Button::new("previous-coefficients")
                        .small()
                        .ghost()
                        .label(crate::text::translate("sourceInspector.previous"))
                        .disabled(self.loading || offset == 0)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.load(offset.saturating_sub(COEFFICIENT_PAGE_ROWS), window, cx)
                        })),
                )
                .child(
                    Button::new("next-coefficients")
                        .small()
                        .ghost()
                        .label(crate::text::translate("sourceInspector.next"))
                        .disabled(self.loading || offset + count >= page.total_count)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.load(offset.saturating_add(COEFFICIENT_PAGE_ROWS), window, cx)
                        })),
                )
            })
    }

    pub(super) fn bars(&self, cx: &App) -> impl IntoElement + use<> {
        let rows = self
            .coefficients
            .as_ref()
            .map(|page| page.coefficients.as_slice())
            .unwrap_or_default();
        let maximum = rows.iter().map(|row| row.coef.abs()).fold(0.001, f64::max);
        div()
            .id("coefficient-bars")
            .max_h(px(360.))
            .overflow_y_scroll()
            .children(rows.iter().enumerate().map(|(index, row)| {
                let positive = row.coef >= 0.;
                let color = if positive {
                    cx.theme().green
                } else {
                    cx.theme().danger
                }
                .opacity(if row.is_significant { 0.7 } else { 0.25 });
                let width = relative((row.coef.abs() / maximum) as f32);
                let minimum = px(if row.coef == 0. { 0. } else { 2. });
                div()
                    .id(("coefficient-bar", index))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new(("coefficient-label", index))
                            .small()
                            .ghost()
                            .w(px(140.))
                            .min_w_0()
                            .label(row.variable.clone())
                            .tooltip(row.variable.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .h(px(20.))
                            .child(div().w_1_2().flex().justify_end().when(!positive, |side| {
                                side.child(
                                    div()
                                        .w(width)
                                        .min_w(minimum)
                                        .h_full()
                                        .bg(color)
                                        .rounded_sm(),
                                )
                            }))
                            .child(div().w(px(1.)).h_full().bg(cx.theme().border))
                            .child(div().w_1_2().when(positive, |side| {
                                side.child(
                                    div()
                                        .w(width)
                                        .min_w(minimum)
                                        .h_full()
                                        .bg(color)
                                        .rounded_sm(),
                                )
                            })),
                    )
                    .child(
                        div()
                            .w(px(85.))
                            .text_xs()
                            .child(display::number(row.coef, 4)),
                    )
            }))
    }
}

fn stars(p: f64) -> &'static str {
    if p < 0.001 {
        "***"
    } else if p < 0.01 {
        "**"
    } else if p < 0.05 {
        "*"
    } else if p < 0.1 {
        "."
    } else {
        ""
    }
}
