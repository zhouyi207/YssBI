use super::SettingsPanel;
use crate::{preferences, text::t};
use gpui_kit::component::setting::SettingGroup;
use gpui_kit::{Context, SharedString};
use std::time::Duration;
use yss_settings::{
    AssistantMode, Autosave, Density, Language, LogLevel, ReasoningEffort, ThemeMode, UserSettings,
};

fn choices<T: Clone>(entries: &[(T, &'static str)]) -> Vec<(T, SharedString)> {
    entries
        .iter()
        .map(|(value, key)| (value.clone(), t(key).into()))
        .collect()
}

impl SettingsPanel {
    pub(super) fn appearance_preferences(&self, cx: &mut Context<Self>) -> SettingGroup {
        SettingGroup::new()
            .item(self.preference_choice(
                "settings.labels.language",
                "settings.descriptions.language",
                |s| s.language,
                |s, v| s.language = v,
                vec![
                    (Language::Chinese, "简体中文".into()),
                    (Language::English, "English".into()),
                ],
                cx,
            ))
            .item(self.preference_choice(
                "preferences.appearance.theme",
                "preferences.appearance.themeHint",
                |s| s.appearance.mode,
                |s, v| s.appearance.mode = v,
                choices(&[
                    (ThemeMode::System, "preferences.system"),
                    (ThemeMode::Light, "preferences.light"),
                    (ThemeMode::Dark, "preferences.dark"),
                ]),
                cx,
            ))
            .item(self.preference_font(
                "preferences.appearance.uiFont",
                "preferences.appearance.uiFontHint",
                |s| s.appearance.ui_font_family.clone(),
                |s, v| s.appearance.ui_font_family = v,
                cx,
            ))
            .item(self.preference_number(
                "preferences.appearance.uiSize",
                "preferences.appearance.uiSizeHint",
                |s| s.appearance.ui_font_size as f64,
                |s, v| s.appearance.ui_font_size = v as f32,
                (8., 32., 1.),
                cx,
            ))
            .item(self.preference_font(
                "preferences.appearance.monoFont",
                "preferences.appearance.monoFontHint",
                |s| s.appearance.mono_font_family.clone(),
                |s, v| s.appearance.mono_font_family = v,
                cx,
            ))
            .item(self.preference_number(
                "preferences.appearance.monoSize",
                "preferences.appearance.monoSizeHint",
                |s| s.appearance.mono_font_size as f64,
                |s, v| s.appearance.mono_font_size = v as f32,
                (8., 48., 1.),
                cx,
            ))
    }

    pub(super) fn document_preferences(&self, cx: &mut Context<Self>) -> SettingGroup {
        SettingGroup::new()
            .item(self.preference_switch(
                "preferences.documents.wrap",
                "preferences.documents.wrapHint",
                |s| s.editor.soft_wrap,
                |s, v| s.editor.soft_wrap = v,
                cx,
            ))
            .item(self.preference_switch(
                "preferences.documents.lineNumbers",
                "preferences.documents.lineNumbersHint",
                |s| s.editor.line_numbers,
                |s, v| s.editor.line_numbers = v,
                cx,
            ))
            .item(self.preference_switch(
                "preferences.documents.preview",
                "preferences.documents.previewHint",
                |s| s.editor.open_in_preview,
                |s, v| s.editor.open_in_preview = v,
                cx,
            ))
            .item(self.preference_choice(
                "preferences.documents.autosave",
                "preferences.documents.autosaveHint",
                |s| s.editor.autosave,
                |s, v| s.editor.autosave = v,
                choices(&[
                    (Autosave::Off, "preferences.off"),
                    (Autosave::AfterDelay, "preferences.documents.afterDelay"),
                    (
                        Autosave::OnFocusChange,
                        "preferences.documents.onFocusChange",
                    ),
                ]),
                cx,
            ))
            .item(self.preference_number(
                "preferences.documents.delay",
                "preferences.documents.delayHint",
                |s| s.editor.autosave_delay_ms as f64,
                |s, v| s.editor.autosave_delay_ms = v.round() as u64,
                (250., 60_000., 250.),
                cx,
            ))
    }

    pub(super) fn data_preferences(&self, cx: &mut Context<Self>) -> SettingGroup {
        SettingGroup::new()
            .item(self.preference_switch(
                "preferences.tables.stripe",
                "preferences.tables.stripeHint",
                |s| s.tables.stripe,
                |s, v| s.tables.stripe = v,
                cx,
            ))
            .item(self.preference_choice(
                "preferences.tables.density",
                "preferences.tables.densityHint",
                |s| s.tables.density,
                |s, v| s.tables.density = v,
                choices(&[
                    (Density::Compact, "preferences.compact"),
                    (Density::Standard, "preferences.standard"),
                    (Density::Comfortable, "preferences.comfortable"),
                ]),
                cx,
            ))
            .item(self.preference_number(
                "preferences.tables.pageSize",
                "preferences.tables.pageSizeHint",
                |s| s.tables.page_size as f64,
                |s, v| s.tables.page_size = v.round() as usize,
                (1., 10_000., 1.),
                cx,
            ))
            .item(self.preference_number(
                "preferences.charts.points",
                "preferences.charts.pointsHint",
                |s| s.charts.max_preview_points as f64,
                |s, v| s.charts.max_preview_points = v.round() as usize,
                (1., 100_000., 100.),
                cx,
            ))
    }

    pub(super) fn workspace_preferences(&self, cx: &mut Context<Self>) -> SettingGroup {
        SettingGroup::new()
            .item(self.preference_switch(
                "preferences.workspace.restoreLayout",
                "preferences.workspace.restoreLayoutHint",
                |s| s.workspace.restore_layout,
                |s, v| s.workspace.restore_layout = v,
                cx,
            ))
            .item(self.preference_switch(
                "preferences.workspace.details",
                "preferences.workspace.defaultsHint",
                |s| s.workspace.show_details,
                |s, v| s.workspace.show_details = v,
                cx,
            ))
            .item(self.preference_switch(
                "preferences.workspace.bottom",
                "preferences.workspace.defaultsHint",
                |s| s.workspace.show_bottom_panel,
                |s, v| s.workspace.show_bottom_panel = v,
                cx,
            ))
            .item(self.preference_switch(
                "preferences.workspace.restoreProject",
                "preferences.workspace.restoreProjectHint",
                |s| s.workspace.restore_last_project,
                |s, v| s.workspace.restore_last_project = v,
                cx,
            ))
    }

    pub(super) fn assistant_preferences(&self, cx: &mut Context<Self>) -> SettingGroup {
        SettingGroup::new()
            .item(self.preference_choice(
                "preferences.assistant.mode",
                "preferences.assistant.modeHint",
                |s| s.assistant.mode,
                |s, v| s.assistant.mode = v,
                choices(&[
                    (AssistantMode::Ask, "panel.assistantModes.ask"),
                    (AssistantMode::Write, "panel.assistantModes.write"),
                ]),
                cx,
            ))
            .item(self.preference_choice(
                "preferences.assistant.effort",
                "preferences.assistant.effortHint",
                |s| s.assistant.reasoning_effort,
                |s, v| s.assistant.reasoning_effort = v,
                choices(&[
                    (None, "preferences.assistant.modelDefault"),
                    (Some(ReasoningEffort::Low), "panel.assistantEffort.low"),
                    (
                        Some(ReasoningEffort::Medium),
                        "panel.assistantEffort.medium",
                    ),
                    (Some(ReasoningEffort::High), "panel.assistantEffort.high"),
                ]),
                cx,
            ))
    }

    pub(super) fn log_preferences(&self, cx: &mut Context<Self>) -> SettingGroup {
        SettingGroup::new()
            .item(self.preference_switch(
                "preferences.logs.follow",
                "preferences.logs.followHint",
                |s| s.logs.auto_scroll,
                |s, v| s.logs.auto_scroll = v,
                cx,
            ))
            .item(self.preference_switch(
                "preferences.logs.error",
                "preferences.logs.filterHint",
                |s| s.logs.visible_levels.contains(&LogLevel::Error),
                |s, v| set_level(s, LogLevel::Error, v),
                cx,
            ))
            .item(self.preference_switch(
                "preferences.logs.warn",
                "preferences.logs.filterHint",
                |s| s.logs.visible_levels.contains(&LogLevel::Warn),
                |s, v| set_level(s, LogLevel::Warn, v),
                cx,
            ))
            .item(self.preference_switch(
                "preferences.logs.info",
                "preferences.logs.filterHint",
                |s| s.logs.visible_levels.contains(&LogLevel::Info),
                |s, v| set_level(s, LogLevel::Info, v),
                cx,
            ))
            .item(self.preference_switch(
                "preferences.logs.debug",
                "preferences.logs.filterHint",
                |s| s.logs.visible_levels.contains(&LogLevel::Debug),
                |s, v| set_level(s, LogLevel::Debug, v),
                cx,
            ))
            .item(self.preference_switch(
                "preferences.logs.trace",
                "preferences.logs.filterHint",
                |s| s.logs.visible_levels.contains(&LogLevel::Trace),
                |s, v| set_level(s, LogLevel::Trace, v),
                cx,
            ))
            .item(self.preference_number(
                "preferences.logs.days",
                "preferences.logs.daysHint",
                |s| s.logs.retention_days.unwrap_or(0) as f64,
                |s, v| s.logs.retention_days = (v >= 1.).then_some(v.round() as u32),
                (0., 3650., 1.),
                cx,
            ))
            .item(self.preference_number(
                "preferences.logs.storage",
                "preferences.logs.storageHint",
                |s| s.logs.max_storage_mib.unwrap_or(0) as f64,
                |s, v| s.logs.max_storage_mib = (v >= 1.).then_some(v.round() as u32),
                (0., 10240., 1.),
                cx,
            ))
    }

    pub(super) fn save_preferences(
        &mut self,
        update: impl FnOnce(&mut UserSettings) + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.task.is_some() || self.knowledge.pending {
            return;
        }
        let mut candidate = self.preference_values(cx).clone();
        update(&mut candidate);
        if candidate == *self.preference_values(cx) && self.preference_error.is_none() {
            return;
        }
        if let Err(error) = candidate
            .validate()
            .and_then(|()| crate::keymap::validate_bindings(&candidate.keybindings, cx))
        {
            tracing::warn!(%error,"Invalid preference change");
            self.report_error(t("preferences.invalid"), false, cx);
            return;
        }
        self.preference_draft = Some(candidate);
        self.preference_revision = self.preference_revision.wrapping_add(1);
        self.preference_error = None;
        if self.preference_write.is_none() {
            self.schedule_preference_save(cx);
        }
        cx.notify();
    }

    fn schedule_preference_save(&mut self, cx: &mut Context<Self>) {
        let timer = cx.background_executor().timer(Duration::from_millis(200));
        self.preference_timer = Some(cx.spawn(async move |view, cx| {
            timer.await;
            if let Err(error) = view.update(cx, |view, cx| view.persist_preferences(cx)) {
                tracing::debug!(%error,"Settings view closed before preference commit");
            }
        }));
    }

    fn persist_preferences(&mut self, cx: &mut Context<Self>) {
        self.preference_timer = None;
        let Some(candidate) = self.preference_draft.clone() else {
            return;
        };
        let revision = self.preference_revision;
        let services = self.services.clone();
        let job = self
            .services
            .executor
            .spawn_blocking(move || -> anyhow::Result<_> {
                let old = services.preferences.snapshot();
                let snapshot = services
                    .preferences
                    .update(move |current| *current = candidate)?;
                let retention_changed = old.logs.retention_days != snapshot.logs.retention_days
                    || old.logs.max_storage_mib != snapshot.logs.max_storage_mib;
                let retention_error = if retention_changed {
                    let policy = yss_logging::LogRetentionPolicy::new(
                        snapshot.logs.retention_days,
                        snapshot.logs.max_storage_mib,
                    )?;
                    match services.logging.logs() {
                        Some(logs) => logs
                            .set_retention(policy)
                            .err()
                            .map(|error| error.to_string()),
                        None => Some("Persistent logging is unavailable".to_owned()),
                    }
                } else {
                    None
                };
                Ok((snapshot, retention_error))
            });
        self.preference_write = Some(cx.spawn(async move |view, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            if let Err(error) = view.update(cx, |view, cx| {
                view.preference_write = None;
                let newer = view.preference_revision != revision;
                if !newer {
                    view.preference_draft = None;
                }
                match result {
                    Ok((snapshot, retention_error)) => {
                        let language_changed =
                            preferences::current(cx).language != snapshot.language;
                        if language_changed {
                            view.initial_page = gpui_kit::component::setting::SelectIndex {
                                page_ix: 2,
                                group_ix: None,
                            };
                        }
                        if let Err(error) = preferences::publish(snapshot, cx) {
                            tracing::warn!(%error,"Saved preferences could not be applied");
                            view.report_error(t("preferences.applyFailed"), false, cx);
                        }
                        if let Some(error) = retention_error {
                            tracing::warn!(%error,"Saved log retention could not be applied");
                            view.report_error(t("preferences.logs.applyFailed"), false, cx);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error,"Could not persist preferences");
                        view.preference_error = Some("native.settings.preferencesSaveFailed");
                        view.report_preference_error(cx);
                    }
                }
                if newer {
                    view.schedule_preference_save(cx);
                }
                cx.notify();
            }) {
                tracing::debug!(%error,"Settings view closed after preference commit");
            }
        }));
    }
}

fn set_level(settings: &mut UserSettings, level: LogLevel, enabled: bool) {
    if enabled && !settings.logs.visible_levels.contains(&level) {
        settings.logs.visible_levels.push(level);
    }
    if !enabled {
        settings
            .logs
            .visible_levels
            .retain(|existing| *existing != level);
    }
}
