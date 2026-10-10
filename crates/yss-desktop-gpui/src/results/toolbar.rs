//! Controls derive page ranges from the accepted page, never from a pending request.
use super::*;
use gpui_kit::Div;
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
};

impl ResultPanel {
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        let paging = self
            .table
            .as_ref()
            .filter(|_| !self.report_mode)
            .map(|table| {
                let page = table.read(cx).delegate();
                (
                    page.offset,
                    page.row_count(),
                    page.total_count,
                    page.has_more,
                )
            });
        let has_toolbar = self.report.is_some() || paging.is_some() || self.loading;
        div().when(has_toolbar, |bar| {
            bar.flex()
                .flex_shrink_0()
                .flex_wrap()
                .items_center()
                .justify_end()
                .gap_2()
                .p_2()
                .text_xs()
                .border_b_1()
                .border_color(cx.theme().border)
                .when(self.report.is_some(), |bar| {
                    bar.children(
                        [
                            (false, "sourceInspector.numericView"),
                            (true, "sourceInspector.reportView"),
                        ]
                        .into_iter()
                        .map(|(report, key)| {
                            Button::new(("result-mode", usize::from(report)))
                                .small()
                                .ghost()
                                .selected(self.report_mode == report)
                                .label(crate::text::translate(key))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.report_mode = report;
                                    cx.notify();
                                }))
                        }),
                    )
                })
                .when(
                    !self.report_mode && self.value.is_some() && self.table.is_some(),
                    |bar| {
                        bar.child(
                            Button::new("result-overview")
                                .small()
                                .ghost()
                                .icon(IconName::ArrowLeft)
                                .label(crate::text::translate("native.results.backToOverview"))
                                .on_click(cx.listener(|view, _, _, cx| view.back_to_overview(cx))),
                        )
                    },
                )
                .when_some(paging, |bar, (offset, count, total, has_more)| {
                    bar.child(row_range(offset, count, total))
                        .child(
                            Button::new("result-prev")
                                .small()
                                .ghost()
                                .icon(IconName::ChevronLeft)
                                .tooltip(crate::text::translate("sourceInspector.previous"))
                                .disabled(self.loading || offset == 0)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.load_page(
                                        view.part.clone(),
                                        offset.saturating_sub(query::PAGE_ROWS),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                        .child(page_range(offset, total))
                        .child(
                            Button::new("result-next")
                                .small()
                                .ghost()
                                .icon(IconName::ChevronRight)
                                .tooltip(crate::text::translate("sourceInspector.next"))
                                .disabled(self.loading || !has_more)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.load_page(
                                        view.part.clone(),
                                        offset.saturating_add(query::PAGE_ROWS),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                })
                .when(self.loading, |bar| {
                    bar.child(crate::text::translate("dataOperation.reading"))
                })
        })
    }

    pub(super) fn read_feedback(&self, cx: &mut Context<Self>) -> Div {
        div().when_some(self.error.as_ref(), |banner, error| {
            banner
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap_3()
                .px_3()
                .py_2()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(crate::text::translate(error.message())),
                )
                .when(error.retryable(), |banner| {
                    banner.child(
                        Button::new("retry-result-read")
                            .small()
                            .ghost()
                            .label(crate::text::translate("common.retry"))
                            .disabled(self.loading)
                            .on_click(
                                cx.listener(|view, _, window, cx| view.retry_read(window, cx)),
                            ),
                    )
                })
        })
    }
}

fn row_range(offset: usize, count: usize, total: Option<usize>) -> String {
    if count == 0 {
        return crate::text::format("detail.counts.rows", &[("count", "0".into())]);
    }
    let mut fields = vec![
        ("start", (offset + 1).to_string()),
        ("end", (offset + count).to_string()),
    ];
    let key = if let Some(total) = total {
        fields.push(("total", total.to_string()));
        "native.results.rowRangeKnown"
    } else {
        "native.results.rowRangeUnknown"
    };
    crate::text::format(key, &fields)
}

fn page_range(offset: usize, total: Option<usize>) -> String {
    let page = offset / query::PAGE_ROWS + 1;
    if let Some(total) = total {
        let pages = total.div_ceil(query::PAGE_ROWS).max(1);
        crate::text::format(
            "native.results.pageKnown",
            &[
                ("page", page.min(pages).to_string()),
                ("pages", pages.to_string()),
            ],
        )
    } else {
        crate::text::format("native.results.pageUnknown", &[("page", page.to_string())])
    }
}
