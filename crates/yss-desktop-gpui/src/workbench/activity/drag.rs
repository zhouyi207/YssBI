//! Drag payloads retain source rows and original catalog descriptors until drop.
use super::*;
use gpui::WeakEntity;
use gpui_component::ActiveTheme;
use yss_node_catalog::ResourceBoundCreateArgs;
use yss_project_identity::ProjectInstanceId;

#[derive(Clone)]
pub(crate) struct ActivityDrag {
    source: WeakEntity<ActivityPanel>,
    document: Arc<ActivityPanelDocument>,
    row: usize,
    creation: Option<(Arc<ActivityPanelDocument>, usize)>,
}

pub(crate) enum ActivityDrop<'a> {
    OpenGraph(&'a str),
    CreateNode(&'a NodeCreation),
}

impl ActivityDrag {
    pub(super) fn new(
        panel: &ActivityPanel,
        row: usize,
        cx: &Context<ActivityPanel>,
    ) -> Option<Self> {
        let document = panel.document.as_ref()?;
        let ActivityRowContent::Item(item) = &document.rows.get(row)?.content else {
            return None;
        };
        let creation = match item {
            ActivityItem::Node {
                available: true, ..
            }
            | ActivityItem::EventGraph { .. } => None,
            ActivityItem::FunctionGraph { path, .. } => {
                Some(panel.resources.as_ref()?.creation(path)?)
            }
            ActivityItem::Database { resource_path, .. } => {
                Some(panel.resources.as_ref()?.creation(resource_path)?)
            }
            _ => return None,
        };
        Some(Self {
            source: cx.entity().downgrade(),
            document: document.clone(),
            row,
            creation,
        })
    }

    pub(crate) fn resolve(
        &self,
        project: &ProjectInstanceId,
        target_path: &str,
        cx: &App,
    ) -> Option<ActivityDrop<'_>> {
        let source = self.source.upgrade()?;
        let source = source.read(cx);
        if self.document.project_instance_id.as_deref() != Some(project.as_str())
            || !source.accepts(&self.document)
        {
            return None;
        }
        let ActivityRowContent::Item(item) = &self.document.rows.get(self.row)?.content else {
            return None;
        };
        if let ActivityItem::EventGraph { path, .. } = item {
            return Some(ActivityDrop::OpenGraph(path));
        }
        let (document, row) = if let Some((catalog, row)) = &self.creation {
            if !Arc::ptr_eq(&source.resources.as_ref()?.catalog, catalog) {
                return None;
            }
            (catalog, *row)
        } else {
            (&self.document, self.row)
        };
        let ActivityRowContent::Item(ActivityItem::Node {
            available: true,
            creation,
            ..
        }) = &document.rows.get(row)?.content
        else {
            return None;
        };
        if let NodeCreation::ResourceBound {
            resource_path,
            create_args: ResourceBoundCreateArgs::FunctionGraph,
            ..
        } = creation
            && resource_path.as_str() == target_path
        {
            return None;
        }
        Some(ActivityDrop::CreateNode(creation))
    }
}

impl Render for ActivityDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = div()
            .p_2()
            .max_w(px(280.))
            .rounded_md()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .border_1()
            .border_color(cx.theme().border);
        match self.document.rows.get(self.row).map(|row| &row.content) {
            Some(ActivityRowContent::Item(ActivityItem::Node {
                title,
                creation,
                available,
                ..
            })) => body.child(crate::catalog_rows::node(title, creation, *available, cx)),
            Some(ActivityRowContent::Item(item)) => body.child(resources::label(item, cx)),
            _ => body,
        }
    }
}
