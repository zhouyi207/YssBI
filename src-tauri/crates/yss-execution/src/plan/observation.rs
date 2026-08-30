#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueRef(u32);

impl ValueRef {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanObservationIntent {
    InspectInput { input: ValueRef },
}
