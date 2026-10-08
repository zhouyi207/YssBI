use super::*;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    emit(
        fragment,
        "decision.turf",
        "TURF combination search",
        "TURF 组合模型",
        vec![
            bounded_user_data_input("criteria", "Option (0/1)", series_type()?, 1, None)?,
            data_output("result", "Best reach", report_type()?)?,
        ],
        vec![positive_integer_parameter("combination_size", 2)?],
    )?;
    let mut ports = ["too_cheap", "cheap", "expensive", "too_expensive"]
        .into_iter()
        .map(|key| data_input(key, key, series_type()?))
        .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?;
    ports.push(data_output(
        "result",
        "Price intersections",
        report_type()?,
    )?);
    ports.push(fixed_numeric_table(
        "curves",
        "Price sensitivity curves",
        &[
            "price",
            "too_cheap",
            "cheap",
            "expensive",
            "too_expensive",
            "not_cheap",
            "not_expensive",
        ],
    )?);
    emit(
        fragment,
        "decision.psm",
        "Price sensitivity meter",
        "PSM 价格敏感度",
        ports,
        vec![choice_parameter(
            "range_definition",
            "original",
            &["original", "narrower"],
        )?],
    )
}
