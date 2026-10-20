//! Forms mount in the existing Details panel; captured targets guard delayed callbacks.
mod parent;

use super::MindCanvas;
use crate::file_session::{FileEditor, file_status};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Textarea,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{Anchor, App, ClickEvent, Context, IntoElement, Window, div, prelude::*};
use std::borrow::Cow;
use yss_project_model::file::FileVersion;
use yss_ui::inspector::{self, InspectorHeader, InspectorSection, PropertyField};
use yss_ui::text::translate;

/// Read-only inspector identity and controls, excluding editable topic text.
#[derive(PartialEq)]
pub struct DetailsDisplay<'a> {
    path: Cow<'a, str>,
    status: &'static str,
    topic: Option<TopicDisplay<'a>>,
    selected_count: usize,
    error: Option<Cow<'a, str>>,
}

/// The selected topic's inspector state; an unapplied draft reveals its discard action.
#[derive(PartialEq)]
struct TopicDisplay<'a> {
    version: Cow<'a, FileVersion>,
    id: Cow<'a, str>,
    collapsed: bool,
    busy: bool,
    dirty: bool,
}

impl DetailsDisplay<'_> {
    pub fn into_owned(self) -> DetailsDisplay<'static> {
        DetailsDisplay {
            path: Cow::Owned(self.path.into_owned()),
            status: self.status,
            topic: self.topic.map(|topic| TopicDisplay {
                version: Cow::Owned(topic.version.into_owned()),
                id: Cow::Owned(topic.id.into_owned()),
                collapsed: topic.collapsed,
                busy: topic.busy,
                dirty: topic.dirty,
            }),
            selected_count: self.selected_count,
            error: self.error.map(|error| Cow::Owned(error.into_owned())),
        }
    }
}

#[derive(Clone)]
struct TopicTarget {
    id: String,
    version: FileVersion,
}
#[derive(Clone, Copy)]
enum TopicAction {
    Apply,
    DiscardInput,
    AddChild,
    AddSibling,
    Up,
    Down,
    Collapse,
    Delete,
}

impl MindCanvas {
    /// Retains only dependencies of the inspector, not the editable topic text.
    pub fn details_state(&self) -> DetailsDisplay<'_> {
        DetailsDisplay {
            path: Cow::Borrowed(self.path()),
            status: file_status(self),
            topic: self.selected_topic().map(|topic| TopicDisplay {
                version: Cow::Borrowed(&self.file.snapshot().version),
                id: Cow::Borrowed(topic.id.as_str()),
                collapsed: self.collapsed.contains(&topic.id),
                busy: !self.file.editable(),
                dirty: self
                    .buffers
                    .get(&topic.id)
                    .is_some_and(|buffer| buffer.dirty),
            }),
            selected_count: self.selected.len(),
            error: self.file.error().map(|error| Cow::Borrowed(error.as_ref())),
        }
    }

    pub fn render_details(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let topic = self.render_topic_section(window, cx);
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                InspectorHeader::new(
                    IconName::Network,
                    self.file.snapshot().path.name().to_owned(),
                )
                .detail(translate("detail.itemTypes.mind")),
            )
            .child(topic)
            .child(
                InspectorSection::new("mind-info", translate("detail.sections.info")).child(
                    inspector::property_list([
                        inspector::property_row(
                            translate("detail.fields.path"),
                            self.path().to_owned(),
                            cx,
                        )
                        .into_any_element(),
                        inspector::property_row(
                            translate("detail.fields.status"),
                            translate(file_status(self)),
                            cx,
                        )
                        .id("mind-status")
                        .role(gpui_kit::accesskit::Role::Status)
                        .into_any_element(),
                    ]),
                ),
            )
            .into_any_element()
    }

    fn render_topic_section(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> InspectorSection {
        let section = InspectorSection::new("mind-topic", translate("detail.sections.topic"));
        let Some(id) = self.selected_topic().map(|topic| topic.id.clone()) else {
            return section.child(inspector::hint(
                if self.selected.is_empty() {
                    translate("native.minds.selectTopicHint")
                } else {
                    yss_ui::text::plural("native.minds.selectedTopics", self.selected.len())
                },
                cx,
            ));
        };
        let Some(input) = self.ensure_buffer(&id, window, cx) else {
            return section;
        };
        let Some(topic) = self.selected_topic() else {
            return section;
        };
        let busy = !self.file.editable();
        let root = topic.parent_id.is_none();
        let dirty = self.buffers.get(&id).is_some_and(|buffer| buffer.dirty);
        let target = TopicTarget {
            id,
            version: self.file.snapshot().version.clone(),
        };
        section
            .child(
                PropertyField::new(translate("native.minds.topicContent"))
                    .when(dirty, |field| {
                        field.action(
                            inspector::action_button(
                                "discard-topic-input",
                                IconName::RotateCcw,
                                translate("native.minds.restoreContent"),
                            )
                            .disabled(busy)
                            .on_click(self.topic_command(
                                TopicAction::DiscardInput,
                                &target,
                                cx,
                            )),
                        )
                    })
                    .control(Textarea::new(&input).small().disabled(busy)),
            )
            .child(
                div().flex().justify_end().child(
                    inspector::commit_button(
                        "apply-topic-content",
                        translate("native.minds.applyContent"),
                        busy,
                    )
                    .on_click(self.topic_command(
                        TopicAction::Apply,
                        &target,
                        cx,
                    )),
                ),
            )
            .when(!root, |section| {
                section.child(
                    PropertyField::new(translate("documents.parent")).control(self.parent_picker(
                        topic,
                        target.clone(),
                        cx,
                    )),
                )
            })
            .when_some(topic.reference.as_ref(), |section, reference| {
                section.child(inspector::property_row(
                    translate("detail.fields.reference"),
                    reference_label(reference),
                    cx,
                ))
            })
            .child(self.topic_commands(&target, root, cx))
    }

    /// Adding topics stays visible; reordering, folding and deletion are secondary commands.
    fn topic_commands(
        &self,
        target: &TopicTarget,
        root: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let busy = !self.file.editable();
        let leaf = !self
            .file
            .snapshot()
            .content
            .nodes
            .iter()
            .any(|node| node.parent_id.as_ref() == Some(&target.id));
        let (collapse_label, collapse_icon) = if self.collapsed.contains(&target.id) {
            (translate("documents.expand"), IconName::ChevronsUpDown)
        } else {
            (translate("documents.collapse"), IconName::ChevronsDownUp)
        };
        let commands = [
            (
                TopicAction::Up,
                translate("conversion.moveUp"),
                IconName::ArrowUp,
                self.sibling_edit(false).is_none(),
            ),
            (
                TopicAction::Down,
                translate("conversion.moveDown"),
                IconName::ArrowDown,
                self.sibling_edit(true).is_none(),
            ),
            (TopicAction::Collapse, collapse_label, collapse_icon, leaf),
        ];
        let owner = cx.entity().downgrade();
        let item = {
            let target = target.clone();
            move |action: TopicAction, label: String, icon: IconName, disabled: bool| {
                let target = target.clone();
                PopupMenuItem::new(label)
                    .icon(icon)
                    .disabled(busy || disabled)
                    .on_click(inspector::owner_command(
                        owner.clone(),
                        move |view: &mut MindCanvas, window, cx| {
                            view.perform(action, &target, window, cx)
                        },
                    ))
            }
        };
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                inspector::add_button("add-child-topic", translate("documents.addChild"))
                    .disabled(busy)
                    .on_click(self.topic_command(TopicAction::AddChild, target, cx)),
            )
            .child(
                Button::new("add-sibling-topic")
                    .small()
                    .ghost()
                    .icon(IconName::Plus)
                    .label(translate("native.minds.addSibling"))
                    .disabled(busy || root)
                    .on_click(self.topic_command(TopicAction::AddSibling, target, cx)),
            )
            .child(div().flex_1())
            .child(
                inspector::overflow_button("topic-actions")
                    .disabled(busy)
                    .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                        for (action, label, icon, disabled) in &commands {
                            menu = menu.item(item(*action, label.clone(), *icon, *disabled));
                        }
                        menu.separator().item(item(
                            TopicAction::Delete,
                            translate("documents.deleteBranch"),
                            IconName::Trash,
                            root,
                        ))
                    }),
            )
    }

    fn accepts_topic(&self, target: &TopicTarget) -> bool {
        self.file.editable()
            && self.file.snapshot().version == target.version
            && self.selected.len() == 1
            && self.selected.contains(&target.id)
    }

    fn topic_command(
        &self,
        action: TopicAction,
        target: &TopicTarget,
        cx: &mut Context<Self>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let target = target.clone();
        cx.listener(move |view, _, window, cx| view.perform(action, &target, window, cx))
    }

    fn perform(
        &mut self,
        action: TopicAction,
        target: &TopicTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_topic(target) {
            return;
        }
        match action {
            TopicAction::Apply => self.apply_edits(vec![], window, cx),
            TopicAction::DiscardInput => self.discard_input(&target.id, window, cx),
            TopicAction::AddChild => self.add_topic(false, window, cx),
            TopicAction::AddSibling => self.add_topic(true, window, cx),
            TopicAction::Up | TopicAction::Down => {
                if let Some(edit) = self.sibling_edit(matches!(action, TopicAction::Down)) {
                    self.apply_edits(vec![edit], window, cx);
                }
            }
            TopicAction::Collapse => self.toggle_branch(target.id.clone(), cx),
            TopicAction::Delete => self.delete_topics(window, cx),
        }
    }
}
fn reference_label(reference: &yss_project_model::mind::MindReference) -> String {
    use yss_project_model::mind::MindReference;
    match reference {
        MindReference::Resource { path } => path.clone(),
        MindReference::GraphNode { path, node_id } => format!("{} · {}", path.as_str(), node_id),
        MindReference::Database { database_id } => database_id.clone(),
    }
}
