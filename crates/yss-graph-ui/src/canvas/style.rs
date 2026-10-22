//! Canvas colors and elevation resolve from theme roles once per frame.
use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::{App, BoxShadow, Hsla};

use super::presentation::State;

/// What a port carries, independently of how a theme colors it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum DataKind {
    Numeric,
    Binary,
    Text,
    Datetime,
    Frame,
    #[default]
    Other,
}

impl DataKind {
    /// Kinds take the theme's base hues, so both appearances keep their contrast.
    pub fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Numeric => theme.cyan,
            Self::Binary => theme.magenta,
            Self::Text => theme.yellow,
            Self::Datetime => theme.green,
            Self::Frame => theme.blue,
            Self::Other => theme.muted_foreground,
        }
    }
}

const KINDS: [DataKind; 6] = [
    DataKind::Numeric,
    DataKind::Binary,
    DataKind::Text,
    DataKind::Datetime,
    DataKind::Frame,
    DataKind::Other,
];

pub(super) struct CanvasStyle {
    pub surface: Hsla,
    pub grid: Hsla,
    pub card: Hsla,
    pub header: Hsla,
    pub border: Hsla,
    pub hover_border: Hsla,
    pub text: Hsla,
    pub muted_text: Hsla,
    pub skeleton: Hsla,
    pub well: Hsla,
    pub selection: Hsla,
    pub selection_fill: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
    pub info: Hsla,
    /// World units: card corners scale with the graph rather than the window.
    pub radius: f32,
    pub resting_shadow: Vec<BoxShadow>,
    pub raised_shadow: Vec<BoxShadow>,
    data: [Hsla; 6],
}

impl CanvasStyle {
    pub fn new(cx: &App) -> Self {
        let theme = cx.theme();
        let dark = theme.is_dark();
        // Light canvases recede behind white cards; dark cards lift slightly above the canvas.
        let (surface, card) = if dark {
            (
                theme.background,
                theme.background.blend(theme.muted.opacity(0.65)),
            )
        } else {
            (theme.muted, theme.background)
        };
        let shadows = theme.shadow_tokens();
        Self {
            surface,
            grid: theme
                .muted_foreground
                .opacity(if dark { 0.28 } else { 0.32 }),
            card,
            header: card.blend(theme.muted.opacity(if dark { 0.45 } else { 0.6 })),
            border: theme.border,
            hover_border: theme.ring,
            text: theme.foreground,
            muted_text: theme.muted_foreground,
            skeleton: theme
                .muted_foreground
                .opacity(if dark { 0.3 } else { 0.22 }),
            well: theme.muted.opacity(if dark { 0.55 } else { 0.7 }),
            selection: theme.list_active_border,
            selection_fill: theme.list_active,
            success: theme.success,
            warning: theme.warning,
            danger: theme.danger,
            info: theme.info,
            radius: f32::from(theme.radius_lg).max(4.),
            resting_shadow: shadows.sm,
            raised_shadow: shadows.lg,
            data: KINDS.map(|kind| kind.color(theme)),
        }
    }

    pub fn data(&self, kind: DataKind) -> Hsla {
        self.data[kind as usize]
    }

    pub fn state(&self, state: State) -> Hsla {
        match state {
            State::Error => self.danger,
            State::Running => self.info,
            State::Stale => self.warning,
            State::Valid | State::Partial => self.success,
            State::Unexecuted => self.muted_text,
        }
    }
}
