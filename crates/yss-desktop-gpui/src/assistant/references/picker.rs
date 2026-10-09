//! A virtual list owns search and keyboard position; the conversation owns attached identities.
use super::*;
use gpui::{Anchor, App, Entity, Focusable, Task, WeakEntity, Window};
use gpui_component::{
    IndexPath,
    list::{List, ListDelegate, ListItem, ListState},
    popover::Popover,
};

struct Picker {
    list: Entity<ListState<Resources>>,
    attached: Vec<ProjectResourceRef>,
}

struct Resources {
    owner: WeakEntity<ConversationPanel>,
    catalog: Option<Arc<ResourceCatalog>>,
    query: String,
    matches: Vec<usize>,
    selected: Option<IndexPath>,
}

impl Resources {
    fn filter(&mut self) {
        self.matches = self
            .catalog
            .as_ref()
            .map_or_else(Vec::new, |catalog| catalog.matches(&self.query));
    }
}

impl ListDelegate for Resources {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.matches.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_owned();
        self.filter();
        // List's immediate selection uses the previous row count. Resolve the
        // first match after it accepts this search, including empty-to-nonempty.
        cx.defer_in(window, |list, window, cx| {
            if list.selected_index().is_none() && !list.delegate().matches.is_empty() {
                list.set_selected_index(Some(IndexPath::default()), window, cx);
                cx.notify();
            }
        });
        Task::ready(())
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let entry = self
            .catalog
            .as_ref()?
            .entries
            .get(*self.matches.get(ix.row)?)?;
        let attached = self
            .owner
            .upgrade()
            .is_some_and(|owner| owner.read(cx).references.contains(&entry.resource));
        Some(
            ListItem::new(("assistant-resource", ix.row))
                .h(px(44.))
                .confirmed(attached)
                .check_icon(IconName::Check)
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .child(div().text_xs().truncate().child(entry.name.clone()))
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(cx.theme().muted_foreground)
                                .child(entry.resource.id.clone()),
                        ),
                ),
        )
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = index;
    }

    fn confirm(&mut self, _: bool, _: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(resource) = self
            .selected
            .and_then(|ix| self.matches.get(ix.row))
            .and_then(|ix| self.catalog.as_ref()?.entries.get(*ix))
            .map(|entry| entry.resource.clone())
        else {
            return;
        };
        let _ = self
            .owner
            .update(cx, |owner, cx| owner.toggle_reference(&resource, cx));
        cx.notify();
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .p_3()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(crate::text::t("panel.assistantNoResources"))
    }
}

impl Picker {
    fn new(
        owner: WeakEntity<ConversationPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let list = cx.new(|cx| {
            ListState::new(
                Resources {
                    owner,
                    catalog: None,
                    query: String::new(),
                    matches: vec![],
                    selected: None,
                },
                window,
                cx,
            )
            .searchable(true)
        });
        Self {
            list,
            attached: vec![],
        }
    }

    fn sync(
        &mut self,
        catalog: Option<&Arc<ResourceCatalog>>,
        attached: &[ProjectResourceRef],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = match (&self.list.read(cx).delegate().catalog, catalog) {
            (Some(old), Some(new)) => !Arc::ptr_eq(old, new),
            (None, None) => false,
            _ => true,
        };
        if !changed && self.attached == attached {
            return;
        }
        self.list.update(cx, |list, cx| {
            if changed {
                list.delegate_mut().catalog = catalog.cloned();
                list.delegate_mut().filter();
                list.set_selected_index(None, window, cx);
            }
            cx.notify();
        });
        self.attached = attached.to_vec();
    }
}

impl ConversationPanel {
    pub(in crate::assistant) fn reference_picker(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let picker = window.use_keyed_state("assistant-resource-picker", cx, |window, cx| {
            Picker::new(owner.clone(), window, cx)
        });
        let list = picker.update(cx, |picker, cx| {
            picker.sync(self.resource_catalog.as_ref(), &self.references, window, cx);
            picker.list.clone()
        });
        let focus = list.focus_handle(cx);
        Popover::new("assistant-resources")
            .anchor(Anchor::BottomLeft)
            .track_focus(&focus)
            .trigger(
                Button::new("assistant-pick-resource")
                    .xsmall()
                    .ghost()
                    .icon(IconName::AtSign)
                    .disabled(self.resource_catalog.is_none())
                    .tooltip(crate::text::t("panel.assistantAttachResource"))
                    .accessibility_label(crate::text::t("panel.assistantAttachResource")),
            )
            .on_open_change(move |open, window, cx| {
                let _ = owner.update(cx, |view, cx| {
                    if !open {
                        view.input.focus_handle(cx).focus(window, cx);
                    }
                });
            })
            .content(move |_, _, cx| {
                div()
                    .w(px(320.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        List::new(&list)
                            .max_h(px(264.))
                            .search_placeholder(crate::text::t("panel.assistantFindResource")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::text::t("panel.assistantReferenceHint")),
                    )
            })
            .into_any_element()
    }
}
