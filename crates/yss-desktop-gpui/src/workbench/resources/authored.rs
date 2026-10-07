//! One native name dialog; each resource retains its original typed creation command.
use crate::workbench::Workbench;
use gpui::{AppContext, Context, Window, prelude::*};
use gpui_component::{
    WindowExt,
    dialog::DialogButtonProps,
    input::{Input, InputState},
};
use yss_project::{
    docs::{DocCommand, DocSnapshot},
    minds::{MindCommand, MindSnapshot},
};
use yss_project_identity::{OperationId, ProjectInstanceId};

#[derive(Clone, Copy)]
pub(in crate::workbench) enum AuthoredKind {
    Document,
    Mind,
    Chart,
}
enum CreatedFile {
    Document(DocSnapshot),
    Mind(MindSnapshot),
    Chart(crate::charts::query::ChartRead),
}

impl Workbench {
    pub(in crate::workbench) fn create_file_dialog(
        &mut self,
        kind: AuthoredKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &self.project else {
            return;
        };
        if self.busy || self.closing {
            return;
        }
        let project = project.identity.clone();
        let lifecycle = self.lifecycle;
        let (title, default_name) = match kind {
            AuthoredKind::Document => ("新建 Markdown 文档", "新建文档"),
            AuthoredKind::Mind => ("新建思维导图", "新建思维导图"),
            AuthoredKind::Chart => ("新建图表", "新建图表"),
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(default_name));
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let input = input.clone();
            let value = input.clone();
            let owner = owner.clone();
            let project = project.clone();
            dialog
                .title(title)
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("创建")
                        .cancel_text("取消")
                        .show_cancel(true),
                )
                .child(Input::new(&input))
                .on_ok(move |_, window, cx| {
                    let name = value.read(cx).value().to_string();
                    if name.trim().is_empty() {
                        return false;
                    }
                    owner
                        .update(cx, |view, cx| {
                            if view.lifecycle != lifecycle
                                || view.busy
                                || view.closing
                                || view
                                    .project
                                    .as_ref()
                                    .is_none_or(|current| current.identity != project)
                            {
                                return false;
                            }
                            view.create_authored_file(
                                kind,
                                project.clone(),
                                name.clone(),
                                window,
                                cx,
                            );
                            true
                        })
                        .unwrap_or(false)
                })
        });
    }
    fn create_authored_file(
        &mut self,
        kind: AuthoredKind,
        project: ProjectInstanceId,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.error = None;
        let lifecycle = self.lifecycle;
        let expected = project.clone();
        let owner = self.services.clone();
        let job = self.services.run(move |services| match kind {
            AuthoredKind::Chart => {
                let receipt = services.application.create_chart_resource(
                    project.clone(),
                    OperationId::new(),
                    name,
                    None,
                )?;
                let path = receipt
                    .deltas
                    .iter()
                    .find_map(|delta| match &delta.payload {
                        yss_project_history::ResourceDocumentPatch::ResourceLifecycle(patch)
                            if patch.before.is_none() =>
                        {
                            patch.after.as_ref().map(|after| after.path.to_string())
                        }
                        _ => None,
                    });
                owner.publish_resource(receipt);
                let path = yss_chart_document::ChartResourcePath::parse(
                    &path.ok_or_else(|| anyhow::anyhow!("created chart path unavailable"))?,
                )?;
                crate::charts::query::read(services, project, path).map(CreatedFile::Chart)
            }
            AuthoredKind::Document => {
                let receipt = services.application.apply_doc_command(
                    project,
                    OperationId::new(),
                    DocCommand::Create { name },
                )?;
                owner.publish_resource(receipt.mutation);
                Ok(CreatedFile::Document(receipt.snapshot.ok_or_else(
                    || anyhow::anyhow!("created document snapshot unavailable"),
                )?))
            }
            AuthoredKind::Mind => {
                let receipt = services.application.apply_mind_command(
                    project,
                    OperationId::new(),
                    MindCommand::Create { name },
                )?;
                owner.publish_resource(receipt.mutation);
                Ok(CreatedFile::Mind(receipt.snapshot.ok_or_else(|| {
                    anyhow::anyhow!("created mind snapshot unavailable")
                })?))
            }
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|current| current.identity != expected)
                {
                    return;
                }
                view.busy = false;
                match result {
                    Some(CreatedFile::Document(snapshot)) => view
                        .install_document(snapshot, window, cx)
                        .update(cx, |document, cx| document.focus_editor(window, cx)),
                    Some(CreatedFile::Mind(snapshot)) => view
                        .install_mind(snapshot, window, cx)
                        .update(cx, |mind, cx| mind.focus_canvas(window, cx)),
                    None => view.error = Some("无法创建文件，请检查名称和当前项目。".into()),
                    Some(CreatedFile::Chart(read)) => view
                        .install_chart(read, window, cx)
                        .update(cx, |chart, cx| chart.focus_chart(window, cx)),
                }
                view.refresh_project(window, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
