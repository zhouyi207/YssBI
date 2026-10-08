use std::sync::Arc;

use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    table::{DataTable, TableState},
};
use yss_application::graph::results::{
    ResultPageKind, ResultPageProjection,
    report::{ResultTablePart, structured::ReportTable},
};
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

use super::section::{Section, Source, Title};
use crate::results::{query::PAGE_ROWS, table::ResultGrid};
use crate::{
    plots::stability::{StabilityData, StabilityPlot},
    services::NativeServices,
};

#[derive(Clone)]
pub(super) enum PageSource {
    Declared(ReportTable),
    Array(ResultTablePart),
    Observations,
    Inline(Arc<[RuntimeValue]>),
}

enum Content {
    Grid(Entity<TableState<ResultGrid>>),
    Roots(Arc<StabilityData>),
    Values(Entity<Section>),
}

pub(super) struct ReportPage {
    services: Arc<NativeServices>,
    reference: ResultReference,
    source: PageSource,
    content: Option<Content>,
    offset: usize,
    count: usize,
    total: Option<usize>,
    has_more: bool,
    requested_offset: usize,
    loading: bool,
    error: bool,
    task: Option<gpui::Task<()>>,
}

impl ReportPage {
    pub fn new(
        services: Arc<NativeServices>,
        reference: ResultReference,
        source: PageSource,
    ) -> Self {
        Self {
            services,
            reference,
            source,
            content: None,
            offset: 0,
            count: 0,
            total: None,
            has_more: false,
            requested_offset: 0,
            loading: false,
            error: false,
            task: None,
        }
    }

    pub fn load(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = false;
        self.requested_offset = offset;
        let reference = self.reference;
        let source = self.source.clone();
        let task = self.services.run(move |services| {
            let page = match &source {
                PageSource::Inline(rows) => {
                    let offset = offset.min(rows.len());
                    let end = offset.saturating_add(PAGE_ROWS).min(rows.len());
                    ResultPageProjection {
                        offset,
                        requested_limit: PAGE_ROWS,
                        total_count: Some(rows.len()),
                        has_more: end < rows.len(),
                        kind: ResultPageKind::Sequence,
                        columns: Box::default(),
                        values: rows[offset..end].to_vec().into_boxed_slice(),
                    }
                }
                source => {
                    let part = match source {
                        PageSource::Declared(table) => table.part().clone(),
                        PageSource::Array(part) => part.clone(),
                        PageSource::Observations => ResultTablePart::Observations,
                        PageSource::Inline(_) => unreachable!("inline page handled above"),
                    };
                    services
                        .application
                        .query_result_table(reference, part, offset, PAGE_ROWS)?
                }
            };
            let (page, roots) = match source {
                PageSource::Observations | PageSource::Inline(_) => (page, None),
                PageSource::Declared(table) => {
                    let content = table.present(page)?;
                    (
                        content.table,
                        content
                            .roots
                            .map(|roots| Arc::new(StabilityData::new(roots))),
                    )
                }
                PageSource::Array(_) => {
                    let mut page = page;
                    page.values = page
                        .values
                        .iter()
                        .map(|row| match row.unannotated() {
                            RuntimeValue::List(cells) if cells.len() == 1 => Ok(cells[0].clone()),
                            _ => Err(anyhow::anyhow!("invalid structured array page")),
                        })
                        .collect::<anyhow::Result<_>>()?;
                    (page, None)
                }
            };
            Ok((page, roots))
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                view.loading = false;
                match result {
                    Ok((page, roots)) => view.install(page, roots, window, cx),
                    Err(_) => view.error = true,
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn install(
        &mut self,
        page: ResultPageProjection,
        roots: Option<Arc<StabilityData>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.offset = page.offset;
        self.count = page.values.len();
        self.total = page.total_count;
        self.has_more = page.has_more;
        self.content = Some(if let Some(roots) = roots {
            Content::Roots(roots)
        } else if matches!(
            self.source,
            PageSource::Declared(_) | PageSource::Observations
        ) {
            Content::Grid(cx.new(|cx| {
                TableState::new(ResultGrid::from_report(page), window, cx).sortable(false)
            }))
        } else if let Some(grid) = ResultGrid::from_structured(&page) {
            Content::Grid(cx.new(|cx| TableState::new(grid, window, cx).sortable(false)))
        } else {
            // The section replaces only this page; nested refs keep their original absolute paths.
            Content::Values(cx.new(|_| {
                Section::new(
                    self.services.clone(),
                    self.reference,
                    Title::Key("reportSections.structuredResult"),
                    Source::Rows(page.values.into(), page.offset),
                    true,
                )
            }))
        });
    }
}

impl Render for ReportPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div().w_full().min_w_0().flex().flex_col().gap_3();
        if let Some(content) = &self.content {
            body = body.child(match content {
                Content::Grid(table) => div()
                    .h(px((self.count.min(10) as f32 + 1.) * 32. + 20.))
                    .child(DataTable::new(table).small().stripe(true))
                    .into_any_element(),
                Content::Roots(data) => div()
                    .w_full()
                    .h(px(400.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::text::translate("native.reports.axes")),
                    )
                    .child(div().flex_1().min_h_0().child(StabilityPlot {
                        data: data.clone(),
                        id: "report-roots".into(),
                        page: self.offset,
                    }))
                    .into_any_element(),
                Content::Values(value) => value.clone().into_any_element(),
            });
            if matches!(content, Content::Roots(_))
                && self.total.is_some_and(|total| total > self.count)
            {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::translate("native.reports.currentPageRoots")),
                );
            }
        }
        body.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_end()
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
                        Button::new("report-retry")
                            .small()
                            .ghost()
                            .label(crate::text::translate("common.retry"))
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.load(view.requested_offset, window, cx)
                            })),
                    )
                })
                .when(self.content.is_some(), |bar| {
                    bar.child(format!(
                        "{}–{} / {}",
                        if self.count == 0 { 0 } else { self.offset + 1 },
                        self.offset + self.count,
                        self.total
                            .map(|total| total.to_string())
                            .unwrap_or_else(|| "?".into())
                    ))
                    .child(
                        Button::new("report-prev")
                            .small()
                            .ghost()
                            .label(crate::text::translate("sourceInspector.previous"))
                            .disabled(self.loading || self.offset == 0)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.load(view.offset.saturating_sub(PAGE_ROWS), window, cx)
                            })),
                    )
                    .child(
                        Button::new("report-next")
                            .small()
                            .ghost()
                            .label(crate::text::translate("sourceInspector.next"))
                            .disabled(self.loading || !self.has_more)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.load(view.offset.saturating_add(PAGE_ROWS), window, cx)
                            })),
                    )
                }),
        )
    }
}
