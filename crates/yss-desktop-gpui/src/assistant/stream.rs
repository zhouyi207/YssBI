//! Subscribe before replay, merge only continuous sequences, and reject stale query delivery.
use super::{ConversationEvent, ConversationPanel, projection::Transcript};
use crate::services::NativeEvent;
use gpui::{Context, Window};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use yss_harness_contract::{HarnessEvent, HarnessEventEnvelope};
use yss_ipc_contract::harness::HarnessEventDto;

impl ConversationPanel {
    pub(super) fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut receiver = self.services.subscribe();
        self.event_task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                let alive = match receiver.recv().await {
                    Ok(NativeEvent::Harness(event)) => view
                        .update_in(cx, |view, window, cx| view.accept_live(event, window, cx))
                        .is_ok(),
                    Ok(NativeEvent::ModelsChanged) => view
                        .update_in(cx, |view, window, cx| view.reload_models(window, cx))
                        .is_ok(),
                    Err(RecvError::Lagged(_)) => view
                        .update_in(cx, |view, window, cx| view.recover(window, cx))
                        .is_ok(),
                    Err(RecvError::Closed) => false,
                    _ => true,
                };
                if !alive {
                    break;
                }
            }
        }));
    }
    fn accept_live(
        &mut self,
        event: HarnessEventEnvelope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.session_id != self.session.id {
            return;
        }
        if self.refreshing {
            if self.buffered.len() < 512 {
                self.buffered.push(event);
            } else {
                self.overflow = true;
            }
            return;
        }
        if !self.ready {
            return;
        }
        let follow = self.transcript_scroll.offset().y + self.transcript_scroll.max_offset().y
            <= gpui::px(32.);
        self.accept_submission(&event);
        let directory_changed = matches!(event.event, HarnessEvent::TurnStarted { .. });
        if self
            .transcript
            .accept(HarnessEventDto::from(&event))
            .is_err()
        {
            self.recover(window, cx);
            return;
        }
        if directory_changed {
            cx.emit(ConversationEvent::DirectoryChanged);
        }
        if !self.transcript.running() {
            self.stopping = false;
        }
        if follow {
            self.transcript_scroll.scroll_to_bottom();
        }
        cx.notify();
    }
    fn recover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.refreshing {
            self.overflow = true;
            return;
        }
        if self.recovering {
            self.ready = false;
            self.stream_error = Some("会话事件仍不连续，请刷新后重试。".into());
            cx.notify();
        } else {
            self.recovering = true;
            self.reload(true, window, cx);
        }
    }
    pub(crate) fn reload(&mut self, recovery: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.selection_generation = self.selection_generation.wrapping_add(1);
        self.selecting = false;
        self.ready = false;
        self.refreshing = true;
        self.buffered.clear();
        self.overflow = false;
        if !recovery {
            self.recovering = false;
        }
        let generation = self.generation;
        let services = self.services.clone();
        let principal = self.principal.clone();
        let id = self.session.id.clone();
        let job = self.services.executor.spawn(async move {
            let session = services
                .application
                .application
                .open_harness_session(&services.application.harness.host, &principal, &id)
                .await
                .map_err(super::commands::session_failure)?;
            let events = services
                .application
                .harness
                .host
                .events_after(&id, 0)
                .await
                .map_err(super::commands::harness_failure)?;
            let catalog = services
                .application
                .harness
                .models
                .catalog()
                .await
                .map_err(|_| "模型目录不可用，请检查模型设置。".to_owned())?;
            services
                .application
                .application
                .validate_harness_session(&services.application.harness.host, &principal, &id)
                .await
                .map_err(super::commands::session_failure)?;
            Ok::<_, String>((session, events, catalog))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .unwrap_or_else(|_| Err("会话读取未完成，请刷新后重试。".into()));
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.refreshing = false;
                match result {
                    Ok((session, events, catalog)) => {
                        let mut candidate = Transcript::default();
                        let mut continuous = !view.overflow;
                        for event in events.iter().chain(view.buffered.iter()) {
                            if event.session_id != session.id
                                || candidate.accept(HarnessEventDto::from(event)).is_err()
                            {
                                continuous = false;
                                break;
                            }
                        }
                        if continuous {
                            for event in &events {
                                view.accept_submission(event);
                            }
                            let buffered = std::mem::take(&mut view.buffered);
                            for event in &buffered {
                                view.accept_submission(event);
                            }
                            view.session = session;
                            if view.transcript.turns.is_empty() {
                                view.transcript_scroll.scroll_to_bottom();
                            }
                            view.transcript = candidate;
                            view.catalog = Some(Arc::new(catalog));
                            view.ready = true;
                            view.stream_error = None;
                            view.recovering = false;
                            view.stopping = false;
                            view.load_resources(window, cx);
                            cx.emit(ConversationEvent::DirectoryChanged);
                        } else {
                            view.stream_error = Some("会话历史存在缺口，请刷新后重试。".into());
                        }
                    }
                    Err(error) => view.stream_error = Some(error),
                }
                view.buffered.clear();
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn reload_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_generation = self.model_generation.wrapping_add(1);
        let generation = self.model_generation;
        let session_generation = self.generation;
        let service = self.services.application.harness.models.clone();
        let job = self
            .services
            .executor
            .spawn(async move { service.catalog().await });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, _, cx| {
                if view.model_generation != generation || view.generation != session_generation {
                    return;
                }
                if let Some(catalog) = result {
                    view.catalog = Some(Arc::new(catalog));
                } else {
                    view.catalog = None;
                    view.error = Some("模型目录读取失败，请检查设置后刷新。".into());
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn accept_submission(&mut self, event: &HarnessEventEnvelope) {
        let Some(pending) = &mut self.pending else {
            return;
        };
        if event.sequence <= pending.after_sequence {
            return;
        }
        if let HarnessEvent::TurnStarted {
            user_message,
            resources,
            ..
        } = &event.event
            && user_message == &pending.message.text
            && resources
                .iter()
                .map(|reference| &reference.resource)
                .eq(pending.message.resources.iter())
        {
            pending.accepted = true;
        }
    }
}
