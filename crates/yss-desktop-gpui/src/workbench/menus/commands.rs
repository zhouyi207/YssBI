//! Menu actions route to existing workbench commands after checking current availability.
use super::{super::Workbench, WorkbenchPanel};
use gpui::{Context, Window};
use gpui_component::dock::DockPlacement;
use yss_graph_document::GraphResourceKind;

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = native_workbench, no_json)]
pub(in crate::workbench) enum MenuCommand {
    NewProject,
    SaveProjectAs,
    CloseProject,
    ImportData,
    NewGraph(GraphResourceKind),
    NewDocument,
    NewMind,
    NewChart,
    ShowPanel(WorkbenchPanel),
    ToggleDock(DockPlacement),
    Exit,
}

impl Workbench {
    pub(in crate::workbench) fn dispatch_menu(
        &mut self,
        command: &MenuCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // An open menu may outlive the state from which its disabled flags were derived.
        if self.is_closing(cx) {
            return;
        }
        match command {
            MenuCommand::NewProject => {
                self.project_form(crate::projects::form::ProjectFormKind::Create, window, cx)
            }
            MenuCommand::SaveProjectAs => {
                self.project_form(crate::projects::form::ProjectFormKind::SaveAs, window, cx)
            }
            MenuCommand::CloseProject => self.close_project(window, cx),
            MenuCommand::ImportData => self.import_dialog(window, cx),
            MenuCommand::NewGraph(kind) => self.create_graph(*kind, window, cx),
            MenuCommand::NewDocument => {
                self.create_file_dialog(super::super::resources::AuthoredKind::Document, window, cx)
            }
            MenuCommand::NewMind => {
                self.create_file_dialog(super::super::resources::AuthoredKind::Mind, window, cx)
            }
            MenuCommand::NewChart => {
                self.create_file_dialog(super::super::resources::AuthoredKind::Chart, window, cx)
            }
            MenuCommand::ShowPanel(panel) => {
                self.show_panel(*panel, window, cx);
            }
            MenuCommand::ToggleDock(placement) => {
                self.dock
                    .update(cx, |dock, cx| dock.toggle_dock(*placement, window, cx));
            }
            MenuCommand::Exit => self.request_close(window, cx),
        }
    }
}
