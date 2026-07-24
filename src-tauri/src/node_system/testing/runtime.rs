use crate::node_system::plan::{
    CompiledResourceRequirement, FunctionPlanHandle, ResourceAccess, ResourceId, ResourceKind,
};

use crate::node_system::runtime::{
    FunctionPlanProvider, Kernel, KernelContext, KernelError, ResourceError, ResourceLease,
    ResourceProvider, RuntimeValue,
};
use std::any::Any;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelRecord {
    pub kernel: Box<str>,
    pub parameters: Box<str>,
    pub inputs: Vec<RuntimeValue>,
    pub outcome: Result<Vec<RuntimeValue>, Box<str>>,
}

#[derive(Debug, Clone, Default)]
pub struct KernelRecorder {
    records: Arc<Mutex<Vec<KernelRecord>>>,
}

impl KernelRecorder {
    pub fn wrap<K: Kernel>(&self, handle: impl Into<Box<str>>, kernel: K) -> RecordingKernel<K> {
        RecordingKernel {
            handle: handle.into(),
            kernel,
            records: self.records.clone(),
        }
    }

    pub fn records(&self) -> Vec<KernelRecord> {
        self.records
            .lock()
            .expect("kernel recorder poisoned")
            .clone()
    }

    pub fn clear(&self) {
        self.records
            .lock()
            .expect("kernel recorder poisoned")
            .clear();
    }
}

pub struct RecordingKernel<K> {
    handle: Box<str>,
    kernel: K,
    records: Arc<Mutex<Vec<KernelRecord>>>,
}

impl<K: Kernel> Kernel for RecordingKernel<K> {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let result = self.kernel.execute(context, inputs);
        self.records
            .lock()
            .expect("kernel recorder poisoned")
            .push(KernelRecord {
                kernel: self.handle.clone(),
                parameters: context.params.as_str().into(),
                inputs: inputs.to_vec(),
                outcome: result
                    .as_ref()
                    .map(|values| values.clone())
                    .map_err(|error| error.0.clone()),
            });
        result
    }
}

#[derive(Debug, Default)]
struct LeakCounts {
    acquired: usize,
    released: usize,
    live: usize,
    attempts: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceLeakTracker {
    counts: Arc<Mutex<LeakCounts>>,
    fail_at_attempt: Option<usize>,
}

impl ResourceLeakTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing_at(attempt: usize) -> Self {
        assert!(attempt > 0, "resource acquisition attempts are one-based");
        Self {
            counts: Arc::new(Mutex::new(LeakCounts::default())),
            fail_at_attempt: Some(attempt),
        }
    }

    pub fn acquired(&self) -> usize {
        self.counts.lock().expect("leak tracker poisoned").acquired
    }

    pub fn released(&self) -> usize {
        self.counts.lock().expect("leak tracker poisoned").released
    }

    pub fn live(&self) -> usize {
        self.counts.lock().expect("leak tracker poisoned").live
    }

    #[track_caller]
    pub fn assert_no_leaks(&self) {
        let counts = self.counts.lock().expect("leak tracker poisoned");
        assert_eq!(
            counts.live, 0,
            "{} resource lease(s) remain live",
            counts.live
        );
        assert_eq!(counts.acquired, counts.released, "resource leases leaked");
    }
}

struct TrackedLease {
    resource: ResourceId,
    counts: Arc<Mutex<LeakCounts>>,
}

impl Drop for TrackedLease {
    fn drop(&mut self) {
        let mut counts = self.counts.lock().expect("leak tracker poisoned");
        counts.released += 1;
        counts.live -= 1;
    }
}

impl ResourceLease for TrackedLease {
    fn resource_id(&self) -> &ResourceId {
        &self.resource
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ResourceProvider for ResourceLeakTracker {
    fn acquire(
        &self,
        requirement: &CompiledResourceRequirement,
    ) -> Result<Box<dyn ResourceLease>, ResourceError> {
        let mut counts = self.counts.lock().expect("leak tracker poisoned");
        counts.attempts += 1;
        if self.fail_at_attempt == Some(counts.attempts) {
            return Err(ResourceError::new("injected acquisition failure"));
        }
        counts.acquired += 1;
        counts.live += 1;
        drop(counts);
        Ok(Box::new(TrackedLease {
            resource: requirement.resource.clone(),
            counts: self.counts.clone(),
        }))
    }
}

pub fn tracked_requirement(name: &str) -> CompiledResourceRequirement {
    CompiledResourceRequirement {
        resource: ResourceId::new(name).expect("valid test resource ID"),
        kind: ResourceKind::TemporaryStorage,
        access: ResourceAccess::Exclusive,
        optional: false,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoFunctionPlans;

impl FunctionPlanProvider for NoFunctionPlans {
    fn get_plan(
        &self,
        _: &FunctionPlanHandle,
    ) -> Result<Option<Arc<crate::node_system::plan::ExecutionPlan>>, Box<str>> {
        Ok(None)
    }
}
