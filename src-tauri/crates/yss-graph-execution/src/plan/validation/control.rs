use std::time::Instant;

#[allow(
    dead_code,
    reason = "validation control is activated by plan admission"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanValidationControl {
    deadline: Instant,
}

impl PlanValidationControl {
    #[allow(
        dead_code,
        reason = "validation control is activated by plan admission"
    )]
    pub const fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    #[allow(
        dead_code,
        reason = "validation control is activated by plan admission"
    )]
    pub const fn deadline(self) -> Instant {
        self.deadline
    }
}
