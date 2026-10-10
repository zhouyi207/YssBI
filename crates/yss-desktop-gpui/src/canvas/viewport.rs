//! Canvas coordinates are live; the workspace retains checkpoints only for reopening.
use super::GraphCanvas;
use crate::services::Viewport;
use gpui::{Context, point, px};

impl GraphCanvas {
    pub(crate) fn bind_viewport(&mut self, project_root: Option<String>) {
        if let Some(viewport) = project_root
            .as_deref()
            .and_then(|root| self.services.layouts.viewport(root, self.path()))
        {
            self.offset = point(px(viewport.x), px(viewport.y));
            self.zoom = viewport.scale;
        }
        self.viewport_root = project_root;
    }

    fn viewport_snapshot(&self) -> Viewport {
        Viewport {
            x: self.offset.x.into(),
            y: self.offset.y.into(),
            scale: self.zoom,
        }
    }

    pub(super) fn checkpoint_viewport(&self, cx: &mut Context<Self>) {
        let Some(root) = &self.viewport_root else {
            return;
        };
        let snapshot = self.viewport_snapshot();
        let Some(job) = self.services.layouts.update_viewport(
            root,
            self.path(),
            snapshot,
            &self.services.executor,
        ) else {
            return;
        };
        let path = self.path().to_owned();
        cx.spawn(async move |view, cx| {
            if !matches!(job.await, Ok(Ok(()))) {
                let _ = view.update(cx, |view, cx| {
                    if view.path() == path && view.viewport_snapshot() == snapshot {
                        view.error =
                            Some(crate::text::translate("native.canvas.viewportSaveFailed"));
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }
}
