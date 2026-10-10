//! One-shot field navigation uses the existing form controls and the Details scroll owner.
use super::ParameterForm;
use gpui_kit::{
    AnyElement, Context, FocusHandle, IntoElement, ScrollHandle, Window, div, point, prelude::*,
};
use std::{cell::Cell, rc::Rc};
use yss_node_protocol::ParameterKey;

#[derive(Clone)]
pub(super) struct Reveal {
    key: ParameterKey,
    scroll: ScrollHandle,
    focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    scheduled: Rc<Cell<bool>>,
}

impl ParameterForm {
    pub fn reveal(
        &mut self,
        key: &ParameterKey,
        scroll: ScrollHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.fields.iter().any(|field| field.model.key == *key) {
            return;
        }
        self.reveal = Some(Reveal {
            key: key.clone(),
            scroll,
            focus: cx.focus_handle().tab_stop(true),
            previous_focus: window.focused(cx),
            scheduled: Default::default(),
        });
        cx.notify();
    }

    pub(super) fn reveal_editor(
        &self,
        index: usize,
        editor: AnyElement,
    ) -> impl IntoElement + use<> {
        div()
            .id(("parameter-editor", index))
            .min_w_0()
            .when_some(
                self.reveal
                    .as_ref()
                    .filter(|target| target.key == self.fields[index].model.key),
                |view, target| view.track_focus(&target.focus),
            )
            .child(editor)
    }

    pub(super) fn reveal_field(
        &self,
        index: usize,
        field: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let epoch = self.epoch;
        div()
            .min_w_0()
            .child(field)
            .when_some(
                self.reveal
                    .as_ref()
                    .filter(|target| target.key == self.fields[index].model.key)
                    .cloned(),
                |view, target| {
                    view.on_children_prepainted(move |bounds, window, _| {
                        let Some(bounds) = bounds.first().copied() else {
                            return;
                        };
                        if target.scheduled.replace(true) {
                            return;
                        }
                        let owner = owner.clone();
                        let target = target.clone();
                        let viewport = target.scroll.bounds();
                        let delta = if bounds.top() < viewport.top()
                            || bounds.size.height > viewport.size.height
                        {
                            viewport.top() - bounds.top()
                        } else if bounds.bottom() > viewport.bottom() {
                            viewport.bottom() - bounds.bottom()
                        } else {
                            gpui_kit::px(0.)
                        };
                        let offset = target.scroll.offset();
                        window.on_next_frame(move |window, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                if view.epoch != epoch
                                    || view
                                        .reveal
                                        .as_ref()
                                        .is_none_or(|current| current.focus != target.focus)
                                {
                                    return;
                                }
                                // Scroll first: off-screen controls are not yet native tab stops.
                                if window.focused(cx) != target.previous_focus
                                    || !(view.can_edit)(cx)
                                {
                                    view.reveal = None;
                                    cx.notify();
                                    return;
                                }
                                target.scroll.set_offset(point(offset.x, offset.y + delta));
                                let owner = cx.entity().downgrade();
                                window.on_next_frame(move |window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.focus_revealed(&target, epoch, window, cx)
                                    });
                                });
                                cx.notify();
                            });
                        });
                    })
                },
            )
            .id(("parameter-field", index))
            .into_any_element()
    }
    fn focus_revealed(
        &mut self,
        target: &Reveal,
        epoch: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.epoch != epoch
            || self
                .reveal
                .as_ref()
                .is_none_or(|current| current.focus != target.focus)
        {
            return;
        }
        self.reveal = None;
        // A new user focus takes precedence over the pending reveal.
        if window.focused(cx) == target.previous_focus && (self.can_edit)(cx) {
            window.focus(&target.focus, cx);
            window.focus_next(cx);
            if !target.focus.contains_focused(window, cx)
                && let Some(previous) = &target.previous_focus
            {
                window.focus(previous, cx);
            }
        }
        cx.notify();
    }
}
