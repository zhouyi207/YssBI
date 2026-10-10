//! Native modal ownership. No backdrop is painted into the originating window.
mod content;
mod forms;
mod view;

pub(crate) use content::ModalContent;
pub(crate) use forms::confirm;
use gpui_kit::base::{
    RootPlugin,
    actions::{Cancel, Confirm},
};
use gpui_kit::component::{Root, TitleBar, WindowExt};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Context, Entity, IntoElement, KeyBinding, Pixels,
    Render, SharedString, Size, Subscription, WeakEntity, Window, WindowBounds, WindowDecorations,
    WindowKind, WindowOptions, div, px, size,
};
use view::ModalWindow;

type Build = Box<dyn FnOnce(&mut Window, &mut App) -> ModalContent>;
struct Pending {
    origin: AnyWindowHandle,
    title: SharedString,
    size: Size<Pixels>,
    build: Build,
}

struct ModalState {
    child: Option<AnyWindowHandle>,
    modal: Option<WeakEntity<ModalWindow>>,
    pending: Option<Pending>,
    opening: bool,
    _activation: Subscription,
}

pub(crate) fn init(cx: &mut App) {
    Root::register_plugin::<ModalState>(cx, ModalState::new);
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, Some("NativeModal")),
        KeyBinding::new("enter", Confirm { secondary: false }, Some("NativeModal")),
    ]);
}

impl ModalState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let activation = cx.observe_window_activation(window, |state, window, cx| {
            state.open_when_active(window, cx);
        });
        cx.on_release(|state, cx| {
            if let Some(child) = state.child.take() {
                cx.defer(move |cx| {
                    let _ = child.update(cx, |_, window, _| window.remove_window());
                });
            }
        })
        .detach();
        Self {
            child: None,
            modal: None,
            pending: None,
            opening: false,
            _activation: activation,
        }
    }

    fn open_when_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !window.is_window_active() || self.opening {
            return;
        }
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.opening = true;
        let parent = window.window_handle();
        let state = cx.weak_entity();
        // GPUI assigns a native dialog's parent from the focused platform window.
        // Wait for activation, then leave the parent's update before opening it.
        cx.defer(move |cx| open_pending(parent, state, pending, cx));
    }
}

impl RootPlugin for ModalState {}
impl Render for ModalState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn state(window: &Window, cx: &App) -> Entity<ModalState> {
    Root::read(window, cx)
        .plugin::<ModalState>()
        .expect("modal windows are initialized")
}

pub(crate) fn open(
    title: impl Into<SharedString>,
    dimensions: Size<Pixels>,
    window: &mut Window,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> ModalContent + 'static,
) {
    request(
        Pending {
            origin: window.window_handle(),
            title: title.into(),
            size: dimensions,
            build: Box::new(build),
        },
        window,
        cx,
    );
}

fn request(pending: Pending, window: &mut Window, cx: &mut App) {
    let state = state(window, cx);
    let child = state.read(cx).child;
    if let Some(child) = child {
        cx.defer(move |cx| {
            let _ = child.update(cx, |_, window, cx| {
                let existing = self::state(window, cx)
                    .read(cx)
                    .modal
                    .clone()
                    .and_then(|modal| modal.upgrade());
                if existing.is_some_and(|modal| modal.read(cx).title == pending.title) {
                    activate_last(window, cx);
                } else {
                    // A nested form can be requested by an event delivered to the workbench.
                    // Its native parent is the visible modal; its business callbacks keep their origin.
                    request(pending, window, cx);
                }
            });
        });
        return;
    }
    state.update(cx, |state, cx| {
        if state.pending.is_some() || state.opening {
            return;
        }
        state.pending = Some(pending);
        state.open_when_active(window, cx);
        if state.pending.is_some() {
            window.activate_window();
        }
    });
}

fn open_pending(
    parent: AnyWindowHandle,
    owner: WeakEntity<ModalState>,
    pending: Pending,
    cx: &mut App,
) {
    let Ok((bounds, focus)) = parent.update(cx, |_, window, cx| {
        let display = window.display(cx);
        let available = display
            .as_ref()
            .map(|display| display.bounds().size)
            .unwrap_or(pending.size);
        let dimensions = size(
            pending.size.width.min(available.width - px(40.)),
            pending.size.height.min(available.height - px(40.)),
        );
        (
            Bounds::centered(display.map(|display| display.id()), dimensions, cx),
            window.focused(cx),
        )
    }) else {
        return;
    };
    let title = pending.title.clone();
    let result = cx.open_window(
        WindowOptions {
            kind: WindowKind::Dialog,
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some(title.clone()),
                ..TitleBar::title_bar_options()
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(320.), px(180.))),
            is_minimizable: false,
            app_id: Some("com.zjy.yssbi".into()),
            ..TitleBar::window_options()
        },
        move |window, cx| {
            let content = (pending.build)(window, cx);
            let modal = cx.new(|cx| {
                ModalWindow::new(title, parent, pending.origin, focus, content, window, cx)
            });
            let root = cx.new(|cx| Root::new(modal.clone(), window, cx));
            root.read(cx)
                .plugin::<ModalState>()
                .unwrap()
                .update(cx, |state, _| {
                    state.modal = Some(modal.downgrade());
                });
            root
        },
    );
    let _ = owner.update(cx, |state, _| {
        state.opening = false;
        state.child = result.as_ref().ok().map(|window| (*window).into());
    });
    if let Err(error) = result {
        tracing::error!(%error, "Native modal window could not be opened");
        let _ = parent.update(cx, |_, window, cx| {
            window.push_notification(crate::text::t("native.modal_window.openFailed"), cx)
        });
    }
}

/// Close this modal after a completed operation, bypassing the cancellation veto.
pub(crate) fn close(window: &mut Window, cx: &mut App) {
    if let Some(modal) = state(window, cx).read(cx).modal.clone() {
        let _ = modal.update(cx, |modal, cx| modal.finish(window, cx));
    }
}

/// The originating window can close its modal after installing a commit receipt.
pub(crate) fn close_child(window: &mut Window, cx: &mut App) {
    let child = detach_last_child(window, cx);
    if let Some(child) = child {
        cx.defer(move |cx| {
            let _ = child.update(cx, |_, window, cx| close(window, cx));
        });
    }
}

fn detach_last_child(window: &Window, cx: &mut App) -> Option<AnyWindowHandle> {
    let state = state(window, cx);
    let child = state.read(cx).child?;
    if let Some(nested) = child
        .update(cx, |_, window, cx| detach_last_child(window, cx))
        .ok()
        .flatten()
    {
        return Some(nested);
    }
    state.update(cx, |state, _| state.child.take())
}

fn activate_last(window: &mut Window, cx: &mut App) {
    if state(window, cx).read(cx).child.is_some() {
        update_child(window, cx, |window, _| window.activate_window());
    } else {
        window.activate_window();
    }
}

pub(crate) fn update_child(
    window: &Window,
    cx: &mut App,
    update: impl FnOnce(&mut Window, &mut App) + 'static,
) {
    let child = state(window, cx).read(cx).child;
    if let Some(child) = child {
        cx.defer(move |cx| {
            let _ = child.update(cx, |_, window, cx| {
                if state(window, cx).read(cx).child.is_some() {
                    update_child(window, cx, update);
                } else {
                    update(window, cx);
                }
            });
        });
    }
}

pub(crate) fn owner_window(window: &Window, cx: &App) -> AnyWindowHandle {
    state(window, cx)
        .read(cx)
        .modal
        .as_ref()
        .and_then(|modal| modal.upgrade())
        .expect("the form belongs to a modal window")
        .read(cx)
        .origin
}

/// Prompts can nest over an existing form; closing the prompt never closes that form.
pub(crate) fn prompt(
    title: &str,
    message: Option<&str>,
    buttons: &[&str],
    window: &mut Window,
    cx: &mut App,
) -> tokio::sync::oneshot::Receiver<usize> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let title: SharedString = title.to_owned().into();
    let message: SharedString = message.unwrap_or_default().to_owned().into();
    let buttons = buttons
        .iter()
        .map(|text| SharedString::from((*text).to_owned()))
        .collect();
    let parent = window.window_handle();
    cx.defer(move |cx| view::open_prompt(parent, title, message, buttons, sender, cx));
    receiver
}
