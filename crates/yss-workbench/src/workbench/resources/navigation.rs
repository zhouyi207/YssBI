//! Resource links share the normal workbench open and lifecycle paths.
use super::super::Workbench;
use gpui_kit::{Context, Window};
use yss_project_identity::{ProjectResourceKind, ProjectResourceRef};

impl Workbench {
    pub(in crate::workbench) fn open_project_resource(
        &mut self,
        resource: &ProjectResourceRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match resource.kind {
            ProjectResourceKind::Database => {
                self.open_database(resource.id.clone(), None, window, cx)
            }
            ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph => {
                self.open_graph(resource.id.clone(), window, cx)
            }
            ProjectResourceKind::Doc | ProjectResourceKind::Mind | ProjectResourceKind::Chart => {
                self.open_file(resource.id.clone(), None, window, cx)
            }
        }
    }
}
