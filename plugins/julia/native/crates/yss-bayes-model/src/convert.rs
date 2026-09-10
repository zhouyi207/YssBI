//! Validated draft-to-spec conversion.

use std::collections::{BTreeMap, BTreeSet};

use super::draft::{BayesModelDraft, SymbolRole};
use super::model::{BayesModelSpec, DatasetRef, ResponseSpec, ValidatedModelParts};
use super::spec_validation::model_spec_is_valid;
use super::validation::ValidationReport;
use super::validators::validate_draft;

pub fn draft_to_model_spec(draft: BayesModelDraft) -> Result<BayesModelSpec, ValidationReport> {
    let report = validate_draft(&draft);
    if !report.is_ok() {
        return Err(report);
    }

    let data_variables = filter_data_bindings(&draft);
    let Some(dataset) = draft.dataset else {
        return Err(report.with_error("dataset_required", "dataset"));
    };
    let Some(response_binding) = draft.response_binding else {
        return Err(report.with_error("response_required", "responseBinding"));
    };
    let Some(response) = draft.bound_response else {
        return Err(report.with_error("response_expression_required", "boundResponse"));
    };
    let Some(predictor) = draft.bound_predictor else {
        return Err(report.with_error("predictor_required", "boundPredictor"));
    };

    let spec = BayesModelSpec::from_validated_parts(ValidatedModelParts {
        dataset: DatasetRef {
            source_type: dataset.source_type,
            source_id: dataset.source_id,
        },
        response: ResponseSpec {
            expression: response,
            data_variables: BTreeMap::from([(response_binding.symbol, response_binding.column)]),
        },
        predictor,
        data_variables,
        likelihood: draft.likelihood,
        parameters: draft.parameters,
        sampler: draft.sampler,
        display_formula: draft.formula_text,
    });
    if !model_spec_is_valid(&spec) {
        return Err(report.with_error("model_spec_invalid", "modelSpec"));
    }
    Ok(spec)
}

fn filter_data_bindings(draft: &BayesModelDraft) -> BTreeMap<String, String> {
    let independent_symbols: BTreeSet<&str> = draft
        .symbols
        .iter()
        .filter(|symbol| symbol.role == SymbolRole::Independent)
        .map(|symbol| symbol.name.as_str())
        .collect();
    draft
        .data_bindings
        .iter()
        .filter(|(symbol, _)| independent_symbols.contains(symbol.as_str()))
        .map(|(symbol, column)| (symbol.clone(), column.clone()))
        .collect()
}
