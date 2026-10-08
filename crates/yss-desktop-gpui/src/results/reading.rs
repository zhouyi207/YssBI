//! Read one immutable result or page; only a successful delivery changes the visible content.
use super::*;

#[derive(Clone)]
pub(super) enum ReadFailure {
    Open,
    Page {
        part: Option<ResultTablePart>,
        offset: usize,
    },
    InvalidPlot,
}

impl ReadFailure {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Open => "native.results.unavailable",
            Self::Page { .. } => "native.results.pageFailed",
            Self::InvalidPlot => "plot.invalidData",
        }
    }

    pub fn retryable(&self) -> bool {
        !matches!(self, Self::InvalidPlot)
    }
}

impl ResultPanel {
    pub fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.closed {
            return;
        }
        self.loading = true;
        self.error = None;
        self.generation += 1;
        let generation = self.generation;
        let reference = self.reference;
        let owner = self.services.clone();
        let task = self.services.run(move |_| query::open(owner, reference));
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.closed || view.generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok((lease, content)) => view.install_content(lease, content, window, cx),
                    Err(_) => view.error = Some(ReadFailure::Open),
                }
                cx.emit(ResultEvent::Loaded(view.loaded()));
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn install_content(
        &mut self,
        lease: ResultLease,
        content: ResultContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reference = lease.reference();
        self.report_subscription = None;
        self.report = None;
        self.plot = None;
        self.value = None;
        self.table = None;
        self.part = None;
        self.expanded.clear();
        self.rows.clear();
        self.tables = false;
        match content {
            ResultContent::InvalidPlot => self.error = Some(ReadFailure::InvalidPlot),
            ResultContent::Plot(data) => self.plot = Some(cx.new(|_| plot::PlotView::new(data))),
            ResultContent::Page(page) => self.install_page(page, window, cx),
            ResultContent::Value {
                value,
                tables,
                report,
            } => {
                if let Some(report) = report {
                    let entity = cx.new(|cx| {
                        report::ReportView::new(
                            self.services.clone(),
                            self.reference,
                            value.clone(),
                            report,
                            window,
                            cx,
                        )
                    });
                    self.report_subscription = Some(cx.subscribe_in(&entity, window, |view, _, request: &yss_application::graph::results::report::addition::LinearSummaryAddition, window, cx| {
                        view.add_contents(request.clone(), window, cx);
                    }));
                    self.report = Some(entity);
                }
                self.tables = tables;
                self.rows = value::rows(&value, &self.expanded, tables);
                self.value = Some(value);
            }
        }
        self.replace_lease(Some(lease));
    }

    pub(super) fn replace_lease(&mut self, lease: Option<ResultLease>) {
        if let Some(previous) = std::mem::replace(&mut self.lease, lease) {
            self.services.run(move |_| {
                drop(previous);
                Ok(())
            });
        }
    }

    fn install_page(&mut self, page: ResultGrid, window: &mut Window, cx: &mut Context<Self>) {
        self.table = Some(cx.new(|cx| TableState::new(page, window, cx).sortable(false)));
    }

    pub(super) fn load_page(
        &mut self,
        part: Option<ResultTablePart>,
        offset: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.closed || self.lease.is_none() {
            return;
        }
        self.loading = true;
        self.error = None;
        self.generation += 1;
        let generation = self.generation;
        let reference = self.reference;
        let owner = self.services.clone();
        let query_part = part.clone();
        let task = self
            .services
            .run(move |_| query::page(&owner, reference, query_part, offset));
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.closed || view.generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(page) => {
                        view.part = part;
                        view.install_page(page, window, cx);
                    }
                    Err(_) => view.error = Some(ReadFailure::Page { part, offset }),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn retry_read(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.error.clone() {
            Some(ReadFailure::Open) => self.load(window, cx),
            Some(ReadFailure::Page { part, offset }) => self.load_page(part, offset, window, cx),
            _ => {}
        }
    }

    pub(super) fn back_to_overview(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        self.task = None;
        self.loading = false;
        self.error = None;
        self.table = None;
        self.part = None;
        cx.notify();
    }
}
