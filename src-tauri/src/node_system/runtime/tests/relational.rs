use super::*;

#[test]
fn invalid_pushdown_plan_is_rejected_before_relational_backend_execution() {
    let executions = Arc::new(Mutex::new(Vec::new()));
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("production", RelationalBackendId::new),
            RecordingRelationalBackend {
                executions: Arc::clone(&executions),
            },
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let mut subplan = relational_subplan("production", "source", Box::new([]));
    subplan.compiled_plan.operators = Box::new([
        RelationalOperator::Source {
            resource: id("database.main", ResourceId::new),
            relation: "items".into(),
        },
        RelationalOperator::Filter {
            input: RelationalOperatorIndex::new(0),
            predicate: RelationalExpression::Literal(RelationalLiteral::Boolean(true)),
        },
        RelationalOperator::Limit {
            input: RelationalOperatorIndex::new(1),
            rows: 25,
        },
    ]);
    subplan.compiled_plan.pushdown_hints = Box::new([RelationalPushdownHint::Limit {
        source: RelationalOperatorIndex::new(0),
        rows: 25,
    }]);
    execution_plan.relational_subplans = Box::new([subplan]);

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&relational)
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::InvalidPlan(_)));
    assert!(executions.lock().unwrap().is_empty());
}

#[test]
fn relational_operation_executes_compiled_subplan_by_index() {
    let executions = Arc::new(Mutex::new(Vec::new()));
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("single", RelationalBackendId::new),
            RecordingRelationalBackend {
                executions: executions.clone(),
            },
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans =
        Box::new([relational_subplan("single", "sales", Box::new([]))]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&relational)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(*executions.lock().unwrap(), vec![Box::<str>::from("sales")]);
    assert_eq!(
        result.value_for_test("result").unwrap(),
        RuntimeValue::from(Value::Integer(41))
    );
}

struct StreamingRelationalBackend {
    observed: Arc<Mutex<Option<StreamValue>>>,
}

impl RelationalBackend for StreamingRelationalBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        let stream = context
            .resource_owner
            .stream_from_values([Value::Integer(1)])
            .map_err(RelationalError::from)?;
        *self.observed.lock().unwrap() = Some(stream.clone());
        Ok(RelationalExecution {
            outputs: vec![RuntimeValue::Stream(stream)],
        })
    }
}

#[test]
fn relational_stream_materializes_and_closes_before_run_cleanup() {
    let observed = Arc::new(Mutex::new(None));
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("single", RelationalBackendId::new),
            StreamingRelationalBackend {
                observed: observed.clone(),
            },
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans =
        Box::new([relational_subplan("single", "sales", Box::new([]))]);

    RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&relational)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert!(observed.lock().unwrap().as_ref().unwrap().is_closed());
}

struct FailingRelationalBackend;

impl RelationalBackend for FailingRelationalBackend {
    fn execute(
        &self,
        _: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        Err(RelationalError::operator_invalid(
            "relational execution failed",
        ))
    }
}

#[test]
fn relational_failure_releases_run_resources_and_backend_lease() {
    let resources = no_resources();
    let released_resources = resources.released.clone();
    let released_backends = Arc::new(AtomicUsize::new(0));
    let provider = TrackingRelationalProvider {
        released: released_backends.clone(),
    };
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.resources = Box::new([requirement("temporary")]);
    execution_plan.relational_subplans =
        Box::new([relational_subplan("single", "sales", Box::new([]))]);

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&provider)
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::RelationalFailed { .. }));
    assert_eq!(released_resources.load(Ordering::SeqCst), 1);
    assert_eq!(released_backends.load(Ordering::SeqCst), 1);
}

struct TrackingRelationalProvider {
    released: Arc<AtomicUsize>,
}

struct TrackingRelationalLease {
    backend: FailingRelationalBackend,
    released: Arc<AtomicUsize>,
}

impl Drop for TrackingRelationalLease {
    fn drop(&mut self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

impl RelationalBackendLease for TrackingRelationalLease {
    fn backend(&self) -> &dyn RelationalBackend {
        &self.backend
    }
}

impl RelationalBackendProvider for TrackingRelationalProvider {
    fn acquire(
        &self,
        _: &RelationalBackendId,
        _: &RunResourceSet,
        _: &CancellationToken,
    ) -> Result<Box<dyn RelationalBackendLease>, RelationalError> {
        Ok(Box::new(TrackingRelationalLease {
            backend: FailingRelationalBackend,
            released: self.released.clone(),
        }))
    }
}
