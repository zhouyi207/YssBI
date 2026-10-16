//! A port's uncommitted text belongs to its native input, independently of graph decoration.
use gpui_kit::component::{ActiveTheme, input::InputState};
use gpui_kit::{App, Entity, Focusable, Pixels, SharedString, Subscription, TextRun, Window, px};
use serde_json::Value;
use yss_data_contract::SemanticType;
use yss_graph_editor::projection::{EditorInputBinding, EditorPortModel};
use yss_project::GraphEditVersion;

pub(super) struct Field {
    pub kind: SemanticType,
    pub element_id: SharedString,
    pub input: Entity<InputState>,
    pub version: GraphEditVersion,
    pub base: Option<EditorInputBinding>,
    pub dirty: bool,
    pub pending: Option<Value>,
    pub skip_blur: bool,
    pub width: Pixels,
    pub error: Option<InputError>,
    pub last_visible: u64,
    pub measurement: Option<(SharedString, SharedString, Pixels)>,
    pub _subscription: Subscription,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum InputError {
    Conflict,
    Number,
}

impl Field {
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.input.read(cx).focus_handle(cx).is_focused(window)
    }

    pub fn install(
        &mut self,
        port: &EditorPortModel,
        version: GraphEditVersion,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.dirty || self.focused(window, cx) {
            if self.base == port.input {
                self.version = version;
            } else {
                self.error = Some(InputError::Conflict);
            }
            return;
        }
        self.restore(port, version, window, cx);
    }

    pub fn restore(
        &mut self,
        port: &EditorPortModel,
        version: GraphEditVersion,
        window: &mut Window,
        cx: &mut App,
    ) {
        let value = text(port, self.kind);
        if self.input.read(cx).value().as_ref() != value {
            self.input
                .update(cx, |input, cx| input.set_value(value, window, cx));
        }
        self.version = version;
        self.base = port.input.clone();
        self.dirty = false;
        self.pending = None;
        self.error = None;
        self.measure(window.text_system(), cx);
    }

    pub fn measure(&mut self, text_system: &gpui_kit::WindowTextSystem, cx: &App) {
        let value = self.input.read(cx).value();
        let value = if value.is_empty() {
            self.input.read(cx).presentation().placeholder().clone()
        } else {
            value
        };
        let font_family = cx.theme().font_family.clone();
        if self
            .measurement
            .as_ref()
            .is_some_and(|(text, family, rem)| {
                text == &value && family == &font_family && *rem == cx.theme().font_size
            })
        {
            return;
        }
        let text = value
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(32)
            .collect::<String>();
        let len = text.len();
        let line = text_system.shape_line(
            text.into(),
            px(10.),
            &[TextRun {
                len,
                font: gpui_kit::font(font_family.clone()),
                color: cx.theme().foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        self.width = (line.width + px(12.)).clamp(px(28.), px(76.));
        self.measurement = Some((value, font_family, cx.theme().font_size));
    }
}

pub(super) fn text(port: &EditorPortModel, kind: SemanticType) -> String {
    let value = port.input.as_ref().and_then(|input| {
        input
            .literal_override
            .as_ref()
            .filter(|value| !value.is_null())
            .or(input.protocol_default.as_ref())
    });
    match value.filter(|value| !value.is_null()) {
        Some(Value::String(value)) => value.clone(),
        Some(value) => value.to_string(),
        None if kind == SemanticType::Numeric => "0".into(),
        None => String::new(),
    }
}

pub(super) fn numeric_draft(value: &str, _: &mut App) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    parts
        .next()
        .is_some_and(|part| part.bytes().all(|ch| ch.is_ascii_digit()))
        && parts
            .next()
            .is_none_or(|part| part.bytes().all(|ch| ch.is_ascii_digit()))
        && parts.next().is_none()
}
