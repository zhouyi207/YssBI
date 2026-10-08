use super::*;

impl Render for PlotView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (x_label, y_label) = match &self.data.geometry {
            Geometry::Cartesian(data) => (
                data.result.x_label.as_deref(),
                data.result.y_label.as_deref(),
            ),
            Geometry::Histogram {
                x_label, y_label, ..
            } => (x_label.as_deref(), y_label.as_deref()),
            _ => (None, None),
        };
        div()
            .id("result-plot")
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .children(self.data.information())
                    .when(self.data.kind == PlotDataKind::Line, |bar| {
                        bar.child(
                            Button::new("line-toolbar")
                                .small()
                                .ghost()
                                .icon(IconName::Settings2)
                                .tooltip(crate::text::translate("native.plots.toolbar"))
                                .selected(self.toolbar_open)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.toolbar_open = !view.toolbar_open;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when(
                self.data.kind == PlotDataKind::Line && self.toolbar_open,
                |view| {
                    view.child(
                        Switch::new("line-points")
                            .small()
                            .label(crate::text::translate("native.plots.points"))
                            .checked(self.points_visible)
                            .on_click(cx.listener(|view, checked: &bool, _, cx| {
                                view.points_visible = *checked;
                                cx.notify();
                            })),
                    )
                },
            )
            .when(
                matches!(
                    self.data.geometry,
                    Geometry::Cartesian(_) | Geometry::Histogram { .. }
                ),
                |view| {
                    view.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::text::format(
                                "native.plots.axes",
                                &[
                                    ("x", x_label.unwrap_or("X").into()),
                                    ("y", y_label.unwrap_or("Y").into()),
                                ],
                            )),
                    )
                },
            )
            .when(self.data.page_count() > 1, |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .child(
                            Button::new("previous-plot-page")
                                .small()
                                .ghost()
                                .icon(IconName::ChevronLeft)
                                .tooltip(crate::text::translate("native.plots.previousPage"))
                                .disabled(self.page == 0)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.page = view.page.saturating_sub(1);
                                    cx.notify();
                                })),
                        )
                        .child(crate::text::format(
                            "native.plots.page",
                            &[
                                ("current", (self.page + 1).to_string()),
                                ("total", self.data.page_count().to_string()),
                                ("count", self.data.paged_count().to_string()),
                            ],
                        ))
                        .child(
                            Button::new("next-plot-page")
                                .small()
                                .ghost()
                                .icon(IconName::ChevronRight)
                                .tooltip(crate::text::translate("native.plots.nextPage"))
                                .disabled(self.page + 1 >= self.data.page_count())
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.page = (view.page + 1).min(view.data.page_count() - 1);
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(match &self.data.geometry {
                Geometry::Cartesian(data) => div()
                    .flex_1()
                    .min_h_0()
                    .child(CartesianPlot {
                        data: data.clone(),
                        id: format!("result-plot-{}", cx.entity_id()).into(),
                        generation: 0,
                        show_points: self.data.kind == PlotDataKind::Line && self.points_visible,
                    })
                    .into_any_element(),
                Geometry::Histogram { bins, .. } => div()
                    .flex_1()
                    .min_h_0()
                    .child(histogram::render(
                        format!("result-histogram-{}", cx.entity_id()),
                        bins,
                        cx,
                    ))
                    .into_any_element(),
                Geometry::Matrix { data, .. } => div()
                    .flex_1()
                    .min_h_0()
                    .child(MatrixPlot {
                        data: data.clone(),
                        id: format!("result-matrix-{}", cx.entity_id()).into(),
                    })
                    .into_any_element(),
                Geometry::Correlogram { acf, pacf, .. } => div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children([(acf, false, "ACF"), (pacf, true, "PACF")].map(
                        |(data, partial, title)| {
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .child(div().text_xs().child(title))
                                .child(div().flex_1().min_h_0().child(Correlogram {
                                    data: data.clone(),
                                    partial,
                                    id: format!("result-{title}-{}", cx.entity_id()).into(),
                                }))
                        },
                    ))
                    .into_any_element(),
                Geometry::Distribution(data) => div()
                    .flex_1()
                    .min_h_0()
                    .child(Distribution {
                        data: data.clone(),
                        id: format!("result-distribution-{}", cx.entity_id()).into(),
                    })
                    .into_any_element(),
                Geometry::Interval { data, .. } => div()
                    .flex_1()
                    .min_h_0()
                    .child(Interval {
                        data: data.clone(),
                        page: self.page,
                        id: format!("result-interval-{}-{}", cx.entity_id(), self.page).into(),
                    })
                    .into_any_element(),
                Geometry::Composite { data, .. } => div()
                    .flex_1()
                    .min_h_0()
                    .child(Composite {
                        data: data.clone(),
                        page: self.page,
                        id: format!("result-composite-{}-{}", cx.entity_id(), self.page).into(),
                    })
                    .into_any_element(),
                Geometry::WordCloud(data) => div()
                    .flex_1()
                    .min_h_0()
                    .child(WordCloud::new(
                        data.clone(),
                        format!("result-wordcloud-{}", cx.entity_id()).into(),
                    ))
                    .into_any_element(),
                Geometry::Nomogram { data, .. } => {
                    div()
                        .id("nomogram-scroll")
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .overflow_scroll()
                        .child(div().min_w(px(640.)).w_full().h(px(data.height())).child(
                            Nomogram {
                                data: data.clone(),
                                id: format!("result-nomogram-{}", cx.entity_id()).into(),
                            },
                        ))
                        .into_any_element()
                }
            })
    }
}
