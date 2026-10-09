//! One native name dialog; each resource retains its original typed creation command.
use super::super::name_form::NameForm;

use crate::workbench::Workbench;
use gpui::{Context, Entity, Window};

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
    Document(Option<DocSnapshot>),
    Mind(Option<MindSnapshot>),
    Chart(Option<crate::charts::query::ChartRead>),
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
        let (title, default_name) = match kind {
            AuthoredKind::Document => (
                crate::text::translate("native.workbench.newMarkdown"),
                crate::text::translate("documents.newDoc"),
            ),
            AuthoredKind::Mind => (
                crate::text::translate("documents.newMind"),
                crate::text::translate("documents.newMind"),
            ),
            AuthoredKind::Chart => (
                crate::text::translate("menubar.newChart"),
                crate::text::translate("menubar.newChart"),
            ),
        };
        self.resource_name_dialog(
            title,
            default_name,
            crate::text::translate("contextMenu.dialog.createSubmit"),
            window,
            cx,
            move |view, name, form, window, cx| {
                view.create_authored_file(kind, project.clone(), name, form, window, cx);
            },
        );
    }
    fn create_authored_file(
        &mut self,
        kind: AuthoredKind,
        project: ProjectInstanceId,
        name: String,
        form: Entity<NameForm>,
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
                let read = path
                    .and_then(|path| yss_chart_document::ChartResourcePath::parse(&path).ok())
                    .and_then(|path| crate::charts::query::read(services, project, path).ok());
                Ok(CreatedFile::Chart(read))
            }
            AuthoredKind::Document => {
                let receipt = services.application.apply_doc_command(
                    project,
                    OperationId::new(),
                    DocCommand::Create { name },
                )?;
                owner.publish_resource(receipt.mutation);
                Ok(CreatedFile::Document(receipt.snapshot))
            }
            AuthoredKind::Mind => {
                let receipt = services.application.apply_mind_command(
                    project,
                    OperationId::new(),
                    MindCommand::Create { name },
                )?;
                owner.publish_resource(receipt.mutation);
                Ok(CreatedFile::Mind(receipt.snapshot))
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
                    NameForm::fail(Some(&form), "native.workbench.resourceChanged", cx);
                    return;
                }
                view.busy = false;
                view.finish_resource_name(
                    Some(form),
                    result
                        .is_none()
                        .then_some("native.workbench.fileCreateFailed"),
                    window,
                    cx,
                );
                if result.is_some() {
                    view.expand_project_category(
                        match kind {
                            AuthoredKind::Document => "project.docs",
                            AuthoredKind::Mind => "project.minds",
                            AuthoredKind::Chart => "project.charts",
                        },
                        cx,
                    );
                }
                match result {
                    Some(CreatedFile::Document(Some(snapshot))) => view
                        .install_document(snapshot, window, cx)
                        .update(cx, |document, cx| document.focus_editor(window, cx)),
                    Some(CreatedFile::Mind(Some(snapshot))) => view
                        .install_mind(snapshot, window, cx)
                        .update(cx, |mind, cx| mind.focus_canvas(window, cx)),
                    None => {}
                    Some(
                        CreatedFile::Document(None)
                        | CreatedFile::Mind(None)
                        | CreatedFile::Chart(None),
                    ) => {
                        view.error = Some(crate::text::translate(
                            "native.workbench.resourceCommittedUnavailable",
                        ));
                    }
                    Some(CreatedFile::Chart(Some(read))) => view
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
