//! Ports and study-table shapes; algorithms do not depend on these node IDs.
use super::*;

pub(super) fn ports(method: &str) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let keys: &[(&str, &str)] = match method {
        "continuous" => &[
            ("treatment_mean", "Treatment mean"),
            ("treatment_sd", "Treatment SD"),
            ("treatment_n", "Treatment N"),
            ("reference_mean", "Reference mean"),
            ("reference_sd", "Reference SD"),
            ("reference_n", "Reference N"),
        ],
        "binary" => &[
            ("treatment_events", "Treatment events"),
            ("treatment_n", "Treatment N"),
            ("reference_events", "Reference events"),
            ("reference_n", "Reference N"),
        ],
        "single_proportion" => &[("events", "Events"), ("sample_size", "Sample size")],
        "mean" => &[
            ("mean", "Mean"),
            ("sd", "Standard deviation"),
            ("sample_size", "Sample size"),
        ],
        "correlation" => &[
            ("correlation", "Correlation"),
            ("sample_size", "Sample size"),
        ],
        "or_hr" => &[
            ("ratio", "Ratio"),
            ("lower", "Lower confidence limit"),
            ("upper", "Upper confidence limit"),
        ],
        "combine_p" => &[("p_values", "P-values")],
        _ => &[("effects", "Effects"), ("variances", "Sampling variances")],
    };
    let mut ports = keys
        .iter()
        .map(|&(key, label)| data_input(key, label, series_type()?))
        .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?;
    if method == "regression" {
        ports.push(bounded_user_data_input(
            "moderators",
            "Moderator",
            series_type()?,
            1,
            None,
        )?);
    }
    if method == "combine_p" {
        ports.push(bounded_user_data_input(
            "weights",
            "Stouffer weights",
            series_type()?,
            0,
            Some(1),
        )?);
    }
    ports.push(data_output(
        "result",
        "Result",
        if matches!(method, "forest" | "funnel") {
            concrete("plot.data")?
        } else {
            report_type()?
        },
    )?);
    let fields: Option<&[&str]> = match method {
        "continuous" | "binary" | "single_proportion" | "mean" | "correlation" | "or_hr" => {
            Some(&[
                "study",
                "effect",
                "variance",
                "standard_error",
                "lower",
                "upper",
            ])
        }
        "fixed_effect" | "random_effect" | "inverse_variance" | "regression" => Some(&[
            "study",
            "effect",
            "variance",
            "standard_error",
            "lower",
            "upper",
            "weight",
            "fitted",
            "residual",
        ]),
        "leave_one_out" | "sensitivity" => Some(&[
            "omitted_study",
            "estimate",
            "standard_error",
            "lower",
            "upper",
            "tau_squared",
            "i_squared_percent",
        ]),
        _ => None,
    };
    if let Some(fields) = fields {
        ports.push(fixed_numeric_table("studies", "Study results", fields)?);
    }
    Ok(ports)
}
