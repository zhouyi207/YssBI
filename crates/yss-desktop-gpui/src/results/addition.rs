//! Install a completed Application report update into the original result panel.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use yss_application::graph::results::report::addition::{
    LinearSummaryAddition, ReportAdditionError,
};

impl ResultPanel {
    pub(super) fn add_contents(
        &mut self,
        request: LinearSummaryAddition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.closed || self.addition_cancel.is_some() || self.lease.is_none() {
            return;
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        self.addition_cancel = Some(cancellation.clone());
        self.addition_feedback(true, None, cx);
        let reference = self.reference;
        let locale = crate::text::locale().to_string();
        let task = self.services.run(move |services| {
            let update = services.application.add_linear_report_contents(
                reference,
                request,
                &locale,
                cancellation,
            )?;
            let value = value::linear_overview(update.report())?;
            Ok((update, value))
        });
        self.addition_task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.closed || view.reference != reference {
                    return;
                }
                view.addition_cancel = None;
                let result = result.and_then(|(update, value)| {
                    let (lease, report) = update.accept(&view.services.application.application)?;
                    Ok((
                        lease,
                        ResultContent::Value {
                            value,
                            tables: true,
                            report: Some(query::ReportContent::Linear(report)),
                        },
                    ))
                });
                match result {
                    Ok((lease, content)) => {
                        let current = lease.reference();
                        // Stop any old numeric-page delivery before changing the panel's identity.
                        view.generation += 1;
                        view.task = None;
                        view.loading = false;
                        view.error = None;
                        view.install_content(lease, content, window, cx);
                        cx.emit(ResultEvent::Replaced {
                            previous: reference,
                            current,
                        });
                    }
                    Err(error) => {
                        let key = match error.downcast_ref::<ReportAdditionError>() {
                            Some(ReportAdditionError::Changed) => "reportSummary.changed",
                            Some(
                                ReportAdditionError::Unavailable | ReportAdditionError::Open(_),
                            ) => "reportSummary.unavailable",
                            _ => "reportSummary.failed",
                        };
                        view.addition_feedback(false, Some(key), cx);
                    }
                }
                cx.notify();
            });
        }));
    }

    fn addition_feedback(&self, busy: bool, error: Option<&'static str>, cx: &mut Context<Self>) {
        if let Some(report) = &self.report {
            report.update(cx, |report, cx| report.set_addition_state(busy, error, cx));
        }
    }

    pub(super) fn cancel_addition(&mut self) {
        if let Some(cancellation) = self.addition_cancel.take() {
            cancellation.store(true, Ordering::Release);
        }
    }
}

impl Drop for ResultPanel {
    fn drop(&mut self) {
        self.cancel_addition();
        self.replace_lease(None);
    }
}
