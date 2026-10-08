use std::sync::Arc;

use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
};
use gpui_kit_assets::IconName;
use yss_application::graph::results::report::structured::{ReportSectionContent, table_reference};
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

use super::page::{PageSource, ReportPage};
use super::{
    analysis::{AnalysisKind, AnalysisView},
    residual::ResidualView,
};
use crate::services::NativeServices;

pub(super) enum Source {
    Equation(String),
    Page(PageSource),
    Value(Arc<RuntimeValue>),
    Rows(Arc<[RuntimeValue]>, usize),
    Analysis(AnalysisKind),
    Residual,
}

pub(super) enum Title {
    Content(String),
    Key(&'static str),
    Array { name: String, count: usize },
    Unavailable(String),
}

impl Title {
    fn display(&self) -> String {
        match self {
            Self::Content(text) => text.clone(),
            Self::Key(key) => crate::text::translate(key),
            Self::Array { name, count } => crate::text::format(
                "native.reports.arrayTitle",
                &[("name", name.clone()), ("count", count.to_string())],
            ),
            Self::Unavailable(name) => {
                crate::text::format("native.reports.unavailableTitle", &[("name", name.clone())])
            }
        }
    }
}

impl From<ReportSectionContent> for Source {
    fn from(content: ReportSectionContent) -> Self {
        match content {
            ReportSectionContent::Equation(text) => Self::Equation(text),
            ReportSectionContent::Table(table) => Self::Page(PageSource::Declared(table)),
        }
    }
}

enum Body {
    Equation(String),
    Page(Entity<ReportPage>),
    Analysis(Entity<AnalysisView>),
    Residual(Entity<ResidualView>),
    Values {
        fields: Vec<(String, String)>,
        children: Vec<Entity<Section>>,
        empty: &'static str,
    },
}

pub(super) struct Section {
    services: Arc<NativeServices>,
    reference: ResultReference,
    title: Title,
    source: Option<Source>,
    open: bool,
    body: Option<Body>,
}

impl Section {
    pub fn new(
        services: Arc<NativeServices>,
        reference: ResultReference,
        title: Title,
        source: Source,
        open: bool,
    ) -> Self {
        Self {
            services,
            reference,
            title,
            source: Some(source),
            open,
            body: None,
        }
    }

    fn prepare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.source.take() else {
            return;
        };
        let source = match source {
            Source::Value(value) => match table_reference(&value) {
                Some((part, _)) => Source::Page(PageSource::Array(part)),
                None => match value.unannotated() {
                    RuntimeValue::List(rows) => Source::Page(PageSource::Inline(rows.clone())),
                    _ => Source::Value(value),
                },
            },
            source => source,
        };
        self.body = Some(match source {
            Source::Analysis(kind) => Body::Analysis(cx.new(|cx| {
                let mut view = AnalysisView::new(self.services.clone(), self.reference, kind);
                view.load(window, cx);
                view
            })),
            Source::Residual => Body::Residual(cx.new(|cx| {
                let mut view = ResidualView::new(self.services.clone(), self.reference, window, cx);
                view.load(window, cx);
                view
            })),
            Source::Equation(text) => Body::Equation(text),
            Source::Page(source) => Body::Page(cx.new(|cx| {
                let mut page = ReportPage::new(self.services.clone(), self.reference, source);
                page.load(0, window, cx);
                page
            })),
            Source::Value(value) => self.values(&value, 0, cx),
            // Already bounded by ReportPage; do not route this page into another pager.
            Source::Rows(values, offset) => self.values(&RuntimeValue::List(values), offset, cx),
        });
    }

    fn values(&self, value: &RuntimeValue, offset: usize, cx: &mut Context<Self>) -> Body {
        let empty = if matches!(value.unannotated(), RuntimeValue::List(_)) {
            "[]"
        } else {
            "{}"
        };
        let entries = match value.unannotated() {
            RuntimeValue::Record(fields) => fields
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect(),
            RuntimeValue::List(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| ((offset + index + 1).to_string(), value.clone()))
                .collect(),
            _ => vec![(
                crate::text::translate("detail.fields.value"),
                (*value).clone(),
            )],
        };
        let mut fields = Vec::new();
        let mut children = Vec::new();
        for (key, value) in entries {
            if matches!(
                value.unannotated(),
                RuntimeValue::Record(_) | RuntimeValue::List(_)
            ) {
                let title = match table_reference(&value) {
                    Some((_, count)) => Title::Array { name: key, count },
                    None => Title::Content(key),
                };
                children.push(cx.new(|_| {
                    Self::new(
                        self.services.clone(),
                        self.reference,
                        title,
                        Source::Value(Arc::new(value)),
                        false,
                    )
                }));
            } else {
                fields.push((key, super::scalar(&value)));
            }
        }
        Body::Values {
            fields,
            children,
            empty,
        }
    }
}

impl Render for Section {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.open {
            self.prepare(window, cx);
        }
        let content = match &self.body {
            Some(Body::Equation(text)) => {
                let source = text.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_family(cx.theme().mono_font_family.clone())
                            .whitespace_normal()
                            .child(text.clone()),
                    )
                    .child(
                        Button::new("copy-report-equation")
                            .small()
                            .ghost()
                            .icon(IconName::Copy)
                            .label(crate::text::translate("menubar.copy"))
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    source.clone(),
                                ))
                            }),
                    )
            }
            Some(Body::Page(page)) => div().child(page.clone()),
            Some(Body::Analysis(view)) => div().child(view.clone()),
            Some(Body::Residual(view)) => div().child(view.clone()),
            Some(Body::Values {
                fields,
                children,
                empty,
            }) => div()
                .flex()
                .flex_col()
                .gap_2()
                .when(fields.is_empty() && children.is_empty(), |body| {
                    body.child(*empty)
                })
                .children(fields.iter().map(|(key, value)| {
                    div()
                        .flex()
                        .gap_4()
                        .py_1()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .w(gpui::relative(0.35))
                                .min_w_0()
                                .text_color(cx.theme().muted_foreground)
                                .child(key.clone()),
                        )
                        .child(div().flex_1().min_w_0().child(value.clone()))
                }))
                .children(children.iter().cloned()),
            None => div(),
        };
        div()
            .w_full()
            .min_w_0()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .p_3()
            .child(
                Collapsible::new()
                    .open(self.open)
                    .child(
                        Button::new("report-section")
                            .small()
                            .ghost()
                            .w_full()
                            .accessibility_label(self.title.display())
                            .tooltip(self.title.display())
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::new(if self.open {
                                            IconName::ChevronDown
                                        } else {
                                            IconName::ChevronRight
                                        })
                                        .size_3(),
                                    )
                                    .child(
                                        div().min_w_0().text_ellipsis().child(self.title.display()),
                                    ),
                            )
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.open = !view.open;
                                cx.notify();
                            })),
                    )
                    .content(div().pt_3().min_w_0().text_sm().child(content)),
            )
    }
}
