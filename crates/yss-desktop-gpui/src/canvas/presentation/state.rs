use crate::{appearance, text};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(in crate::canvas) enum State {
    #[default]
    Unexecuted,
    Running,
    Error,
    Valid,
    Stale,
    Partial,
}

impl State {
    pub fn resolve(cache: Self, running: bool, error: bool) -> Self {
        if error {
            Self::Error
        } else if running {
            Self::Running
        } else {
            cache
        }
    }

    pub fn label(self) -> &'static str {
        text::t(match self {
            Self::Unexecuted => "canvas.graphState.unexecuted",
            Self::Running => "canvas.graphState.running",
            Self::Error => "canvas.graphState.error",
            Self::Valid => "canvas.graphState.valid",
            Self::Stale => "canvas.graphState.stale",
            Self::Partial => "canvas.graphState.partial",
        })
    }

    pub fn color(self) -> u32 {
        match self {
            Self::Error => appearance::RED,
            Self::Running | Self::Stale => appearance::AMBER,
            Self::Valid | Self::Partial => appearance::GREEN,
            Self::Unexecuted => appearance::MUTED,
        }
    }

    pub fn dashes(self) -> &'static [f32] {
        match self {
            Self::Unexecuted => &[3., 6.],
            Self::Stale => &[8., 4., 2., 4.],
            Self::Error => &[2., 3.],
            Self::Running | Self::Valid | Self::Partial => &[],
        }
    }

    pub fn opacity(self) -> f32 {
        if matches!(self, Self::Unexecuted | Self::Stale) {
            0.6
        } else {
            1.
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(in crate::canvas) struct CacheCount {
    pub total: usize,
    pub valid: usize,
    pub stale: usize,
}

impl CacheCount {
    pub fn add(&mut self, state: State) {
        self.total += 1;
        self.valid += usize::from(state == State::Valid);
        self.stale += usize::from(state == State::Stale);
    }

    pub fn state(self) -> State {
        if self.total > 0 && self.valid == self.total {
            State::Valid
        } else if self.valid > 0 {
            State::Partial
        } else if self.stale > 0 {
            State::Stale
        } else {
            State::Unexecuted
        }
    }
}
