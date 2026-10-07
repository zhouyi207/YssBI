use crate::services::NativeServices;
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window, div,
    prelude::*, px, uniform_list,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
};
use std::{collections::VecDeque, sync::Arc};
use tokio::sync::mpsc;
use yss_logging::{LogBatchDto, LogLevel, LogRecordDto, LogRuntime, LogStreamFailure};

const VIEW_CAPACITY: usize = 1_000;

struct LogLease {
    runtime: LogRuntime,
    id: String,
    executor: tokio::runtime::Handle,
}
impl Drop for LogLease {
    fn drop(&mut self) {
        let runtime = self.runtime.clone();
        let id = self.id.clone();
        self.executor.spawn_blocking(move || {
            let _ = runtime.unsubscribe(id);
        });
    }
}

pub struct LogsPanel {
    services: Arc<NativeServices>,
    focus: FocusHandle,
    entries: VecDeque<LogRecordDto>,
    stream: String,
    sequence: u64,
    epoch: u64,
    lease: Option<LogLease>,
    task: Option<gpui::Task<()>>,
    connecting: bool,
    error: Option<&'static str>,
}

impl LogsPanel {
    pub fn new(services: Arc<NativeServices>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            services,
            focus: cx.focus_handle(),
            entries: VecDeque::new(),
            stream: String::new(),
            sequence: 0,
            epoch: 0,
            lease: None,
            task: None,
            connecting: false,
            error: None,
        };
        panel.connect(window, cx);
        panel
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.task = None;
        self.lease = None;
        self.epoch += 1;
        let Some(runtime) = self.services.logging.logs().cloned() else {
            self.error = Some("日志存储不可用");
            cx.notify();
            return;
        };
        let epoch = self.epoch;
        let executor = self.services.executor.clone();
        let (sender, mut receiver) = mpsc::channel(64);
        self.connecting = true;
        self.error = None;
        let task = self.services.executor.spawn_blocking(move || {
            let snapshot =
                runtime.subscribe_batches(move |batch| sender.try_send(batch).is_ok())?;
            let lease = LogLease {
                runtime,
                id: snapshot.subscription_id.clone(),
                executor,
            };
            Ok::<_, yss_logging::LogsUnavailable>((snapshot, lease))
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result.map_err(anyhow::Error::from));
            let Ok((snapshot, lease)) = result else {
                let _ = view.update(cx, |view, cx| {
                    if view.epoch == epoch {
                        view.connecting = false;
                        view.error = Some("无法连接日志存储");
                        cx.notify();
                    }
                });
                return;
            };
            let accepted = view
                .update(cx, |view, cx| {
                    if view.epoch != epoch {
                        return false;
                    }
                    view.connecting = false;
                    view.error = None;
                    view.stream = snapshot.stream_id;
                    view.sequence = snapshot.latest_sequence;
                    view.entries = snapshot
                        .entries
                        .into_iter()
                        .rev()
                        .take(VIEW_CAPACITY)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    view.lease = Some(lease);
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !accepted {
                return;
            }
            while let Some(batch) = receiver.recv().await {
                let keep = view
                    .update_in(cx, |view, window, cx| {
                        if view.epoch != epoch {
                            return false;
                        }
                        view.accept_batch(batch, window, cx)
                    })
                    .unwrap_or(false);
                if !keep {
                    return;
                }
            }
            let _ = view.update_in(cx, |view, window, cx| {
                if view.epoch == epoch {
                    view.lease = None;
                    // A full host queue closes this sink. Recover from committed recent history.
                    cx.defer_in(window, |view, window, cx| view.connect(window, cx));
                }
            });
        }));
        cx.notify();
    }

    fn accept_batch(
        &mut self,
        batch: LogBatchDto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(failure) = batch.failure {
            self.lease = None;
            match failure {
                LogStreamFailure::StorageUnavailable => self.error = Some("日志存储不可用"),
                LogStreamFailure::SubscriberLagged => {
                    cx.defer_in(window, |view, window, cx| view.connect(window, cx))
                }
            }
            cx.notify();
            return false;
        }
        if batch.stream_id != self.stream
            || batch
                .entries
                .first()
                .is_some_and(|entry| entry.sequence > self.sequence + 1)
        {
            self.lease = None;
            cx.defer_in(window, |view, window, cx| view.connect(window, cx));
            return false;
        }
        for entry in batch.entries {
            if entry.sequence <= self.sequence {
                continue;
            }
            self.sequence = entry.sequence;
            self.entries.push_back(entry);
        }
        while self.entries.len() > VIEW_CAPACITY {
            self.entries.pop_front();
        }
        cx.notify();
        true
    }
}

impl Render for LogsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .gap_1()
            .bg(cx.theme().background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("refresh-logs")
                            .small()
                            .ghost()
                            .icon(IconName::Replace)
                            .label("刷新")
                            .disabled(self.connecting)
                            .on_click(cx.listener(|view, _, window, cx| view.connect(window, cx))),
                    )
                    .child(
                        Button::new("clear-logs-view")
                            .small()
                            .ghost()
                            .icon(IconName::Delete)
                            .label("清空显示")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.entries.clear();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} 条", self.entries.len())),
                    ),
            )
            .when(!self.entries.is_empty(), |view| {
                view.child(
                    uniform_list(
                        "logs-list",
                        self.entries.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                            range
                                .filter_map(|index| view.entries.get(index))
                                .map(|entry| {
                                    let color = match entry.level {
                                        LogLevel::Error => cx.theme().danger,
                                        LogLevel::Warn => cx.theme().warning,
                                        _ => cx.theme().foreground,
                                    };
                                    div()
                                        .h(px(24.))
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_3()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_xs()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .w(px(92.))
                                                .flex_shrink_0()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(
                                                    entry
                                                        .timestamp
                                                        .get(11..23)
                                                        .unwrap_or(&entry.timestamp)
                                                        .to_owned(),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .w(px(45.))
                                                .flex_shrink_0()
                                                .text_color(color)
                                                .child(format!("{:?}", entry.level)),
                                        )
                                        .child(entry.message.clone())
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .flex_1()
                    .min_h_0(),
                )
            })
            .when(
                self.entries.is_empty() && !self.connecting && self.error.is_none(),
                |view| {
                    view.child(crate::appearance::empty_state(
                        IconName::SquareTerminal,
                        "暂无日志",
                        "应用运行记录会显示在这里",
                        cx,
                    ))
                },
            )
            .when(self.connecting, |view| view.child("正在连接…"))
            .when_some(self.error, |view, error| {
                view.child(div().text_color(cx.theme().danger).child(error))
            })
    }
}
impl EventEmitter<PanelEvent> for LogsPanel {}
impl Focusable for LogsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for LogsPanel {
    fn panel_name(&self) -> &'static str {
        "logs"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for LogsPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::SquareTerminal).size_3())
            .child("日志")
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
