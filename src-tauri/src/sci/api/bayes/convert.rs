use std::collections::{BTreeMap, BTreeSet};

use super::draft::{BayesModelDraft, SymbolRole};
use super::model::{BayesModelSpec, DatasetRef, ResponseSpec};
use super::validation::ValidationReport;
use super::validators::validate_draft;

pub fn draft_to_model_spec(draft: BayesModelDraft) -> Result<BayesModelSpec, ValidationReport> {
    let report = validate_draft(&draft);
    if !report.ok {
        return Err(report);
    }

    let data_variables = filter_data_bindings(&draft);
    let dataset = draft.dataset.expect("validated dataset");
    let response_binding = draft.response_binding.expect("validated response binding");
    let response_symbol = response_binding
        .symbol
        .clone()
        .or(draft.response_symbol.clone())
        .unwrap_or_else(|| "y".to_string());
    let predictor = draft.bound_predictor.expect("validated predictor");

    Ok(BayesModelSpec {
        dataset: DatasetRef {
            source_type: dataset.source_type,
            source_id: dataset.source_id,
        },
        response: ResponseSpec {
            symbol: response_symbol,
            column: response_binding.column,
        },
        predictor,
        data_variables,
        likelihood: draft.likelihood,
        parameters: draft.parameters,
        sampler: draft.sampler,
        display_formula: draft.formula_text,
    })
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
