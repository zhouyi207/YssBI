//! Resolve paths with the captured project before handing them to the native platform.
use super::super::Workbench;
use gpui::{Context, Window};
use yss_project::RevealProjectResourceRequest;

impl Workbench {
    pub(in crate::workbench) fn reveal_resource(
        &mut self,
        request: RevealProjectResourceRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        let Some(project) = self
            .project
            .as_ref()
            .map(|project| project.identity.clone())
        else {
            return;
        };
        let expected = project.clone();
        let lifecycle = self.lifecycle;
        let job = self.services.run(move |services| {
            Ok(services
                .application
                .reveal_project_resource(project, request)?)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, _, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != expected)
                {
                    return;
                }
                if let Some(path) = result {
                    cx.reveal_path(&path);
                } else {
                    view.error = Some(crate::text::translate(
                        "native.workbench.revealResourceFailed",
                    ));
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
