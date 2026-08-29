use super::{ActivationId, ActivationResultGroup, RunError, StoredValue};
use crate::execution::plan::legacy::{AttemptId, OperationIndex, WorkloadClass};
use std::collections::VecDeque;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulingPolicy {
    pub cpu_parallelism: NonZeroUsize,
    pub io_parallelism: NonZeroUsize,
    pub adapter_parallelism: NonZeroUsize,
}

impl SchedulingPolicy {
    pub(super) fn worker_count(self) -> usize {
        self.cpu_parallelism
            .get()
            .saturating_add(self.io_parallelism.get())
            .saturating_add(self.adapter_parallelism.get())
    }
}

impl Default for SchedulingPolicy {
    fn default() -> Self {
        let cpu = std::thread::available_parallelism()
            .unwrap_or_else(|_| NonZeroUsize::new(1).expect("one is nonzero"));
        Self {
            cpu_parallelism: cpu,
            io_parallelism: NonZeroUsize::new(4).expect("four is nonzero"),
            adapter_parallelism: NonZeroUsize::new(2).expect("two is nonzero"),
        }
    }
}

#[derive(Debug)]
pub struct OperationCompletion {
    pub operation: OperationIndex,
    pub activation: ActivationId,
    pub attempt: AttemptId,
    pub output_group: Option<ActivationResultGroup>,
    pub outputs: Result<Box<[StoredValue]>, RunError>,
}

const ADMISSION_ORDER: [WorkloadClass; 5] = [
    WorkloadClass::Cpu,
    WorkloadClass::Cpu,
    WorkloadClass::Io,
    WorkloadClass::AdapterIo,
    WorkloadClass::Exclusive,
];

pub(super) struct ClassScheduler {
    policy: SchedulingPolicy,
    cpu: VecDeque<OperationIndex>,
    io: VecDeque<OperationIndex>,
    adapter: VecDeque<OperationIndex>,
    exclusive: VecDeque<OperationIndex>,
    running_cpu: usize,
    running_io: usize,
    running_adapter: usize,
    running_exclusive: bool,
    cursor: usize,
}

impl ClassScheduler {
    pub(super) fn new(policy: SchedulingPolicy) -> Self {
        Self {
            policy,
            cpu: VecDeque::new(),
            io: VecDeque::new(),
            adapter: VecDeque::new(),
            exclusive: VecDeque::new(),
            running_cpu: 0,
            running_io: 0,
            running_adapter: 0,
            running_exclusive: false,
            cursor: 0,
        }
    }

    pub(super) fn enqueue(&mut self, operation: OperationIndex, class: WorkloadClass) {
        self.queue_mut(class).push_back(operation);
    }

    pub(super) fn admit(&mut self) -> Option<(OperationIndex, WorkloadClass)> {
        if self.running_exclusive {
            return None;
        }
        if !self.exclusive.is_empty() && self.running_count() > 0 {
            return None;
        }
        for _ in 0..ADMISSION_ORDER.len() {
            let class = ADMISSION_ORDER[self.cursor];
            self.cursor = (self.cursor + 1) % ADMISSION_ORDER.len();
            if !self.can_admit(class) {
                continue;
            }
            let operation = self.queue_mut(class).pop_front()?;
            self.mark_admitted(class);
            return Some((operation, class));
        }
        None
    }

    pub(super) fn release(&mut self, class: WorkloadClass) {
        match class {
            WorkloadClass::Cpu => self.running_cpu -= 1,
            WorkloadClass::Io => self.running_io -= 1,
            WorkloadClass::AdapterIo => self.running_adapter -= 1,
            WorkloadClass::Exclusive => self.running_exclusive = false,
        }
    }

    #[cfg(test)]
    pub(super) fn blocked_class(&self) -> Option<WorkloadClass> {
        if !self.exclusive.is_empty() && self.running_count() > 0 {
            return Some(WorkloadClass::Exclusive);
        }
        ADMISSION_ORDER
            .iter()
            .copied()
            .find(|class| !self.queue(*class).is_empty() && !self.can_admit(*class))
    }

    pub(super) fn has_queued(&self) -> bool {
        !self.cpu.is_empty()
            || !self.io.is_empty()
            || !self.adapter.is_empty()
            || !self.exclusive.is_empty()
    }

    pub(super) fn running_count(&self) -> usize {
        self.running_cpu
            + self.running_io
            + self.running_adapter
            + usize::from(self.running_exclusive)
    }

    fn can_admit(&self, class: WorkloadClass) -> bool {
        if self.queue(class).is_empty() {
            return false;
        }
        match class {
            WorkloadClass::Cpu => self.running_cpu < self.policy.cpu_parallelism.get(),
            WorkloadClass::Io => self.running_io < self.policy.io_parallelism.get(),
            WorkloadClass::AdapterIo => {
                self.running_adapter < self.policy.adapter_parallelism.get()
            }
            WorkloadClass::Exclusive => self.running_count() == 0,
        }
    }

    fn mark_admitted(&mut self, class: WorkloadClass) {
        match class {
            WorkloadClass::Cpu => self.running_cpu += 1,
            WorkloadClass::Io => self.running_io += 1,
            WorkloadClass::AdapterIo => self.running_adapter += 1,
            WorkloadClass::Exclusive => self.running_exclusive = true,
        }
    }

    fn queue(&self, class: WorkloadClass) -> &VecDeque<OperationIndex> {
        match class {
            WorkloadClass::Cpu => &self.cpu,
            WorkloadClass::Io => &self.io,
            WorkloadClass::AdapterIo => &self.adapter,
            WorkloadClass::Exclusive => &self.exclusive,
        }
    }

    fn queue_mut(&mut self, class: WorkloadClass) -> &mut VecDeque<OperationIndex> {
        match class {
            WorkloadClass::Cpu => &mut self.cpu,
            WorkloadClass::Io => &mut self.io,
            WorkloadClass::AdapterIo => &mut self.adapter,
            WorkloadClass::Exclusive => &mut self.exclusive,
        }
    }
}
