use super::*;

#[test]
fn meta_admission_accounts_for_workspace_and_output_after_inputs_fit() {
    let relations = crate::tests::relations();
    let values = [
        series(&[0.2, 0.4, 0.1, 0.7]),
        series(&[0.04, 0.09, 0.05, 0.16]),
    ];
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let mut control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let id = KernelId::new("yssbi.statistics.meta.cochran_q".into()).unwrap();
    for budget in [128 * 1024 * 1024, 16 * 1024] {
        control.max_input_bytes = budget;
        let inv = KernelInvocation {
            relations: &relations,
            inputs: &values,
            input_keys: &["effects", "variances"],
            parameters: Default::default(),
            outputs: &outputs,
            control: &control,
        };
        // Materialization fits both budgets; the second must fail in aggregate workspace admission.
        super::super::common::materialize(&inv).unwrap();
        let result = KernelRegistry::default().execute(&id, &inv);
        if budget == 16 * 1024 {
            assert!(matches!(result, Err(KernelError::BudgetExceeded)));
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
    }
}
