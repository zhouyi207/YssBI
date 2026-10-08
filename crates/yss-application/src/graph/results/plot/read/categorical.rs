use super::*;
use yss_sci_contract::visualization::{
    CombinationPlot, ParetoCategory, ParetoPlot, WordCloudPlot, WordCount,
};

fn word(raw: &Record) -> Result<WordCount, Invalid> {
    let label = text(field(raw, "label")?)?;
    if label.trim().is_empty() {
        return Err(invalid());
    }
    Ok(WordCount {
        label: label.into(),
        count: positive_count(field(raw, "count")?)?,
    })
}

pub(super) fn word_cloud(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let observations = positive_count(field(raw, "observations")?)?;
    let unique_words = positive_count(field(raw, "uniqueWords")?)?;
    let words = rows(raw, "words", 1)?
        .iter()
        .map(|value| word(object(value)?))
        .collect::<Result<Vec<_>, _>>()?;
    let displayed_observations = words
        .iter()
        .try_fold(0usize, |total, word| total.checked_add(word.count));
    if unique_words < words.len() || displayed_observations.is_none_or(|count| count > observations)
    {
        return Err(invalid());
    }
    Ok(ResultPlotProjection::WordCloud(WordCloudPlot {
        words,
        observations,
        unique_words,
    }))
}

pub(super) fn pareto(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let observations = positive_count(field(raw, "observations")?)?;
    let data = rows(raw, "data", 1)?
        .iter()
        .map(|value| {
            let raw = object(value)?;
            let word = word(raw)?;
            let cumulative = positive(field(raw, "cumulative")?)?;
            if cumulative > 1. {
                return Err(invalid());
            }
            Ok(ParetoCategory {
                label: word.label,
                count: word.count,
                cumulative,
            })
        })
        .collect::<Result<Vec<_>, Invalid>>()?;
    if data.last().map(|row| row.cumulative) != Some(1.)
        || data
            .windows(2)
            .any(|pair| pair[0].cumulative > pair[1].cumulative)
        || data
            .iter()
            .try_fold(0usize, |total, row| total.checked_add(row.count))
            != Some(observations)
    {
        return Err(invalid());
    }
    Ok(ResultPlotProjection::Pareto(ParetoPlot {
        data,
        observations,
    }))
}

pub(super) fn combination(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let labels = rows(raw, "labels", 1)?
        .iter()
        .map(|value| text(value).map(String::from))
        .collect::<Result<Vec<_>, _>>()?;
    let bars = rows(raw, "bars", 1)?
        .iter()
        .map(number)
        .collect::<Result<Vec<_>, _>>()?;
    let line = rows(raw, "line", 1)?
        .iter()
        .map(number)
        .collect::<Result<Vec<_>, _>>()?;
    let RuntimeValue::Scalar(TabularScalar::Bool(dual_axis)) =
        field(raw, "dualAxis")?.unannotated()
    else {
        return Err(invalid());
    };
    let metadata = metadata(field(raw, "metadata")?)?;
    if labels.len() != bars.len()
        || bars.len() != line.len()
        || metadata.displayed != labels.len()
        || metadata.displayed > metadata.observations
    {
        return Err(invalid());
    }
    Ok(ResultPlotProjection::Combination(CombinationPlot {
        labels,
        bars,
        line,
        dual_axis: *dual_axis,
        metadata,
    }))
}
