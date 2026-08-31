use std::sync::{Arc, RwLock};

type TestHook = Arc<dyn Fn() + Send + Sync>;
type TestHookSlot = Arc<RwLock<Option<TestHook>>>;

#[cfg(test)]
pub(super) type GraphLoadAfterReadTestHook = TestHook;
#[cfg(test)]
pub(super) type VariableStagingTestHook = TestHook;
pub type ProjectActivationTestHook = TestHook;
pub type ActivationPublicationTestHook = TestHook;

#[derive(Default)]
pub(crate) struct ProjectStateTestHooks {
    #[cfg(test)]
    pub(crate) graph_load_after_read_test_hook: TestHookSlot,
    pub(crate) variable_staging_test_hook: TestHookSlot,
    pub(crate) project_activation_test_hook: TestHookSlot,
    pub(crate) activation_store_replaced_test_hook: TestHookSlot,
}
