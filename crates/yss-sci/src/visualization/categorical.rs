use super::*;
use std::collections::BTreeMap;
use yss_sci_contract::visualization::*;

fn counts(labels: &[String], control: &ScientificExecutionControl) -> Result<Vec<WordCount>> {
    control.check()?;
    if labels.is_empty() {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    let mut counts = BTreeMap::<&str, usize>::new();
    for (index, label) in labels.iter().enumerate() {
        if index.is_multiple_of(1024) {
            control.check()?;
        }
        let label = label.trim();
        if label.is_empty() {
            return Err(invalid(ScientificInputViolation::EmptyInput));
        }
        *counts.entry(label).or_default() += 1;
    }
    let mut counts = counts
        .into_iter()
        .map(|(label, count)| WordCount {
            label: label.into(),
            count,
        })
        .collect::<Vec<_>>();
    counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    control.check()?;
    Ok(counts)
}

pub fn word_cloud(
    words: &[String],
    max_words: usize,
    control: &ScientificExecutionControl,
) -> Result<WordCloudPlot> {
    if !(1..=MAX_PLOT_WORDS).contains(&max_words) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let mut counts = counts(words, control)?;
    let unique_words = counts.len();
    counts.truncate(max_words);
    Ok(WordCloudPlot {
        words: counts,
        observations: words.len(),
        unique_words,
    })
}

pub fn pareto(labels: &[String], control: &ScientificExecutionControl) -> Result<ParetoPlot> {
    let counts = counts(labels, control)?;
    let mut cumulative = 0usize;
    let data = counts
        .into_iter()
        .map(|value| {
            cumulative += value.count;
            ParetoCategory {
                label: value.label,
                count: value.count,
                cumulative: cumulative as f64 / labels.len() as f64,
            }
        })
        .collect();
    Ok(ParetoPlot {
        data,
        observations: labels.len(),
    })
}

pub fn combination(
    labels: &[String],
    bars: &[f64],
    line: &[f64],
    dual_axis: bool,
    control: &ScientificExecutionControl,
) -> Result<CombinationPlot> {
    let length = aligned(&[bars, line], control)?;
    if labels.len() != length {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    let indices = sample_indices(length, MAX_PLOT_POINTS);
    Ok(CombinationPlot {
        labels: indices.iter().map(|i| labels[*i].clone()).collect(),
        bars: indices.iter().map(|i| bars[*i]).collect(),
        line: indices.iter().map(|i| line[*i]).collect(),
        dual_axis,
        metadata: sample_metadata(length),
    })
}
