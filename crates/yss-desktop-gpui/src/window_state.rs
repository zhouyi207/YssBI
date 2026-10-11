use crate::services::LayoutStore;
use gpui_kit::component::{WindowExt, notification::Notification};
use gpui_kit::{App, AppContext, Context, Entity, Pixels, Size, Task, Window, px, size};
use std::sync::Arc;
use tokio::runtime::Handle;

pub(crate) struct WindowState {
    store: Option<Arc<LayoutStore>>,
    size: Size<Pixels>,
    executor: Handle,
    load_failed: bool,
    changed: bool,
    delivery: Option<Task<()>>,
}

impl WindowState {
    pub(crate) fn load(executor: Handle) -> Self {
        let mut state = Self {
            store: None,
            size: size(px(1480.), px(940.)),
            executor,
            load_failed: false,
            changed: false,
            delivery: None,
        };
        let loaded = LayoutStore::for_application().and_then(|store| {
            let store = Arc::new(store);
            let saved = store.window_bounds();
            state.store = Some(store);
            if let Some(saved) = saved? {
                anyhow::ensure!(valid_size(saved), "Invalid saved window size");
                state.size = size(saved.width.max(px(900.)), saved.height.max(px(620.)));
            }
            Ok(())
        });
        if let Err(error) = loaded {
            tracing::warn!(%error, "Main window size could not be restored");
            state.load_failed = true;
        }
        state
    }

    pub(crate) fn size(&self) -> Size<Pixels> {
        self.size
    }

    pub(crate) fn attach(self, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let state = cx.new(|cx| {
            cx.observe_window_bounds(window, |state: &mut Self, window, cx| {
                state.resized(window, cx);
            })
            .detach();
            cx.on_release(|state: &mut Self, _| {
                // Queue the final blocking write before the runtime shuts down, even
                // when the window closes while its resize debounce is still pending.
                drop(state.save(false));
            })
            .detach();
            cx.on_app_quit(|state: &mut Self, _| {
                let job = state.save(false);
                async move {
                    if let Some(job) = job {
                        match job.await {
                            Ok(Ok(())) => {}
                            result => {
                                tracing::warn!(?result, "Main window size could not be saved")
                            }
                        }
                    }
                }
            })
            .detach();
            self
        });
        if state.read(cx).load_failed {
            window.defer(cx, |window, cx| {
                window.push_notification(
                    Notification::error(crate::text::translate(
                        "native.workbench.layoutRestoreFailed",
                    )),
                    cx,
                );
            });
        }
        state
    }

    fn resized(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_maximized() || window.is_fullscreen() {
            return;
        }
        // Window creation adds client-decoration insets again. Persisting the
        // outer bounds would grow the window on every restart on Linux.
        let size = window.inner_window_bounds().get_bounds().size;
        if !valid_size(size) || size == self.size {
            return;
        }
        self.size = size;
        self.changed = true;
        if let Some(job) = self.save(true) {
            self.delivery = Some(cx.spawn_in(window, async move |state, cx| {
                if !matches!(job.await, Ok(Ok(())))
                    && let Err(error) = state.update_in(cx, |_, window, cx| {
                        window.push_notification(
                            Notification::error(crate::text::translate(
                                "native.workbench.layoutSaveFailed",
                            )),
                            cx,
                        );
                    })
                {
                    tracing::warn!(%error, "Window size save failure could not be displayed");
                }
            }));
        }
    }

    fn save(&self, debounce: bool) -> Option<tokio::task::JoinHandle<anyhow::Result<()>>> {
        if !self.changed {
            return None;
        }
        self.store
            .as_ref()
            .map(|store| store.save_window_bounds(self.size, debounce, &self.executor))
    }
}

fn valid_size(size: Size<Pixels>) -> bool {
    f32::from(size.width).is_finite()
        && f32::from(size.height).is_finite()
        && size.width > px(0.)
        && size.height > px(0.)
}
