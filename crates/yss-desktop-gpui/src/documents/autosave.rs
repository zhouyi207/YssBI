//! Document-only autosave. Timers belong to the editor; disk writes use captured save commands.
use super::DocumentEditor;
use gpui_kit::{Context, Window};
use std::time::Duration;
use yss_settings::{Autosave, EditorSettings};

pub(super) struct AutosaveState {
    pub settings: EditorSettings,
    blocked: bool,
    focus_pending: bool,
}

impl AutosaveState {
    pub fn new(settings: EditorSettings) -> Self {
        Self {
            settings,
            blocked: false,
            focus_pending: false,
        }
    }

    /// Presentation changes must not re-arm a failed save or change the document's mode.
    pub fn configure(&mut self, settings: EditorSettings) -> bool {
        let changed = self.settings.autosave != settings.autosave
            || self.settings.autosave_delay_ms != settings.autosave_delay_ms;
        self.settings = settings;
        if changed {
            self.blocked = false;
            self.focus_pending = false;
        }
        changed
    }

    pub fn edited(&mut self) {
        self.blocked = false;
    }

    pub fn focus_lost(&mut self, dirty: bool) {
        if self.settings.autosave == Autosave::OnFocusChange && dirty && !self.blocked {
            self.focus_pending = true;
        }
    }

    pub fn can_save(&self, dirty: bool, busy: bool) -> bool {
        dirty && !busy && !self.blocked && self.settings.autosave != Autosave::Off
    }

    pub fn delay(&self, dirty: bool) -> Option<Duration> {
        (dirty && !self.blocked && self.settings.autosave == Autosave::AfterDelay)
            .then(|| Duration::from_millis(self.settings.autosave_delay_ms))
    }

    pub fn focus_save_ready(&self, dirty: bool, busy: bool) -> bool {
        self.focus_pending && self.can_save(dirty, busy)
    }

    pub fn save_started(&mut self) {
        self.focus_pending = false;
        self.blocked = false;
    }

    pub fn failed(&mut self) {
        // No timer-driven retries. Another edit, an autosave-policy change, or an
        // explicit save is required before trying again, including edits that
        // arrived while the failed write was in flight.
        self.blocked = true;
        self.focus_pending = false;
    }
}

impl DocumentEditor {
    pub(super) fn apply_preferences(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = crate::preferences::current(cx).editor.clone();
        let wrap_changed = settings.soft_wrap != self.autosave.settings.soft_wrap;
        let numbers_changed = settings.line_numbers != self.autosave.settings.line_numbers;
        if wrap_changed || numbers_changed {
            self.input.update(cx, |input, cx| {
                if wrap_changed {
                    input.set_soft_wrap(settings.soft_wrap, window, cx);
                }
                if numbers_changed {
                    input.set_line_number(settings.line_numbers, window, cx);
                }
            });
        }
        if self.autosave.configure(settings) {
            self.autosave_task = None;
            self.resume_autosave(window, cx);
        }
        // open_in_preview is deliberately read only by new(). Editor typography
        // is supplied by the component's live theme, not a second font snapshot.
        cx.notify();
    }

    pub(super) fn schedule_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.autosave_task = None;
        let Some(delay) = self.autosave.delay(self.dirty()) else {
            return;
        };
        let timer = cx.background_executor().timer(delay);
        self.autosave_task = Some(cx.spawn_in(window, async move |view, cx| {
            timer.await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.autosave_task = None;
                // An in-flight save/refresh resumes the pending work on completion;
                // never spin or start a second write against the same revision.
                if view.autosave.can_save(view.dirty(), view.busy()) {
                    view.save_document(true, window, cx);
                }
            });
        }));
    }

    pub(super) fn autosave_on_focus_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.autosave.focus_lost(self.dirty());
        if self.autosave.focus_save_ready(self.dirty(), self.busy()) {
            self.save_document(true, window, cx);
        }
    }

    pub(super) fn resume_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if self.autosave.focus_save_ready(self.dirty(), false) {
            self.save_document(true, window, cx);
        } else {
            self.schedule_autosave(window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(mode: Autosave) -> EditorSettings {
        let mut settings = EditorSettings::default();
        settings.autosave = mode;
        settings.autosave_delay_ms = 725;
        settings
    }

    #[test]
    fn off_never_schedules_or_saves_dirty_documents() {
        let mut state = AutosaveState::new(settings(Autosave::Off));
        state.edited();
        state.focus_lost(true);
        assert_eq!(state.delay(true), None);
        assert!(!state.can_save(true, false));
        assert!(!state.focus_save_ready(true, false));
    }

    #[test]
    fn delay_uses_configured_milliseconds_only_for_dirty_documents() {
        let state = AutosaveState::new(settings(Autosave::AfterDelay));
        assert_eq!(state.delay(true), Some(Duration::from_millis(725)));
        assert_eq!(state.delay(false), None);
        assert!(!state.can_save(true, true));
        assert!(state.can_save(true, false));
    }

    #[test]
    fn focus_change_waits_for_an_in_flight_save_without_losing_new_edits() {
        let mut state = AutosaveState::new(settings(Autosave::OnFocusChange));
        state.save_started();
        state.edited();
        assert!(!state.focus_save_ready(true, false));
        state.focus_lost(true);
        assert!(!state.focus_save_ready(true, true));
        assert!(state.focus_save_ready(true, false));
        state.save_started();
        assert!(!state.focus_save_ready(true, false));
        assert_eq!(state.delay(true), None);
    }

    #[test]
    fn failed_save_does_not_retry_even_if_edits_arrived_during_the_write() {
        let mut state = AutosaveState::new(settings(Autosave::AfterDelay));
        state.save_started();
        state.edited();
        state.failed();
        assert_eq!(state.delay(true), None);
        assert!(!state.can_save(true, false));
        state.edited();
        assert_eq!(state.delay(true), Some(Duration::from_millis(725)));
    }

    #[test]
    fn failed_focus_save_requires_an_edit_or_explicit_retry() {
        let mut state = AutosaveState::new(settings(Autosave::OnFocusChange));
        state.failed();
        state.focus_lost(true);
        assert!(!state.focus_save_ready(true, false));
        state.save_started();
        state.edited();
        state.focus_lost(true);
        assert!(state.focus_save_ready(true, false));
        assert!(!state.focus_save_ready(false, false));
    }

    #[test]
    fn policy_changes_invalidate_pending_focus_saves() {
        let mut state = AutosaveState::new(settings(Autosave::OnFocusChange));
        state.focus_lost(true);
        assert!(state.configure(settings(Autosave::Off)));
        assert!(!state.focus_save_ready(true, false));
        assert!(state.configure(settings(Autosave::OnFocusChange)));
        assert!(!state.focus_save_ready(true, false));
    }

    #[test]
    fn presentation_changes_do_not_rearm_failed_autosaves() {
        let mut state = AutosaveState::new(settings(Autosave::AfterDelay));
        state.failed();
        let mut changed = state.settings.clone();
        changed.open_in_preview = true;
        changed.soft_wrap = false;
        changed.line_numbers = false;
        assert!(!state.configure(changed));
        assert_eq!(state.delay(true), None);
        let mut changed = state.settings.clone();
        changed.autosave_delay_ms = 950;
        assert!(state.configure(changed));
        assert_eq!(state.delay(true), Some(Duration::from_millis(950)));
    }
}
