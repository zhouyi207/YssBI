using LinearAlgebra

const GAMMA_EPS = 3.0e-14
const GAMMA_FPMIN = 1.0e-300
const GAMMA_ITMAX = 200

function parse_lags(value, observation_count::Int)
    value isa Integer || throw(ArgumentError("`parameters.lags` must be an integer"))
    lags = try
        Int(value)
    catch
        throw(ArgumentError("`parameters.lags` is outside the supported range"))
    end
    1 <= lags <= observation_count - 1 || throw(ArgumentError(
        "`parameters.lags` must be between 1 and $(observation_count - 1)"
    ))
    return lags
end

function parse_bool(value, name::String, default::Bool)
    value === nothing && return default
    value isa Bool || throw(ArgumentError("`$name` must be a boolean"))
    return value
end

function optional_string_vector(value, name::String)
    value === nothing && return String[]
    value isa AbstractVector || throw(ArgumentError("`$name` must be an array"))
    result = String[]
    for item in value
        item isa AbstractString || throw(ArgumentError("`$name` must contain only strings"))
        push!(result, String(item))
    end
    return result
end

function serial_numeric_column(table, column_name::String, task_id::String)
    column_symbol = Symbol(column_name)
    hasproperty(table, column_symbol) || throw(ArgumentError("column `$column_name` was not found"))
    column = getproperty(table, column_symbol)
    values = Vector{Float64}(undef, length(column))

    for index in eachindex(column)
        check_cancelled(task_id)
        value = column[index]
        value isa Real && !(value isa Bool) || throw(ArgumentError(
            "column `$column_name` must contain only numeric values"
        ))
        numeric_value = Float64(value)
        isfinite(numeric_value) || throw(ArgumentError(
            "column `$column_name` must contain only finite values"
        ))
        values[index] = numeric_value
    end

    length(values) >= 4 || throw(ArgumentError("column `$column_name` must contain at least 4 observations"))
    return values
end

function serial_numeric_matrix(table, column_names::Vector{String}, row_count::Int, task_id::String)
    isempty(column_names) && return nothing
    matrix = Matrix{Float64}(undef, row_count, length(column_names))
    for (column_index, column_name) in enumerate(column_names)
        values = serial_numeric_column(table, column_name, task_id)
        length(values) == row_count || throw(ArgumentError("exog column `$column_name` length does not match residuals"))
        matrix[:, column_index] = values
    end
    return matrix
end

function log_gamma(value::Float64)
    value > 0.0 || throw(ArgumentError("log-gamma input must be positive"))
    coefficients = (
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    )
    if value < 0.5
        return log(pi) - log(sin(pi * value)) - log_gamma(1.0 - value)
    end
    z = value - 1.0
    x = 0.99999999999980993
    for (index, coefficient) in enumerate(coefficients)
        x += coefficient / (z + index)
    end
    t = z + length(coefficients) - 0.5
    return 0.5 * log(2.0 * pi) + (z + 0.5) * log(t) - t + log(x)
end

function gamma_p_series(a::Float64, x::Float64)
    gln = log_gamma(a)
    x <= 0.0 && return 0.0
    ap = a
    sum_value = 1.0 / a
    delta = sum_value
    for _ in 1:GAMMA_ITMAX
        ap += 1.0
        delta *= x / ap
        sum_value += delta
        abs(delta) < abs(sum_value) * GAMMA_EPS && return sum_value * exp(-x + a * log(x) - gln)
    end
    throw(ArgumentError("gamma series did not converge"))
end

function gamma_q_contfrac(a::Float64, x::Float64)
    gln = log_gamma(a)
    b = x + 1.0 - a
    c = 1.0 / GAMMA_FPMIN
    d = 1.0 / max(b, GAMMA_FPMIN)
    h = d
    for i in 1:GAMMA_ITMAX
        an = -Float64(i) * (Float64(i) - a)
        b += 2.0
        d = an * d + b
        abs(d) < GAMMA_FPMIN && (d = GAMMA_FPMIN)
        c = b + an / c
        abs(c) < GAMMA_FPMIN && (c = GAMMA_FPMIN)
        d = 1.0 / d
        delta = d * c
        h *= delta
        abs(delta - 1.0) < GAMMA_EPS && return exp(-x + a * log(x) - gln) * h
    end
    throw(ArgumentError("gamma continued fraction did not converge"))
end

function gamma_q(a::Float64, x::Float64)
    a > 0.0 || throw(ArgumentError("gamma shape must be positive"))
    x < 0.0 && throw(ArgumentError("gamma x must be non-negative"))
    x == 0.0 && return 1.0
    if x < a + 1.0
        return clamp(1.0 - gamma_p_series(a, x), 0.0, 1.0)
    end
    return clamp(gamma_q_contfrac(a, x), 0.0, 1.0)
end

function chisq_sf(stat::Float64, df::Int)
    stat >= 0.0 || return 1.0
    df > 0 || throw(ArgumentError("chi-square degrees of freedom must be positive"))
    return gamma_q(df / 2.0, stat / 2.0)
end

function durbin_watson_stat(residuals::Vector{Float64})
    n = length(residuals)
    n < 2 && return 2.0
    sum_sq_diff = 0.0
    for index in 2:n
        diff = residuals[index] - residuals[index - 1]
        sum_sq_diff += diff * diff
    end
    sum_sq = sum(value * value for value in residuals)
    sum_sq <= 0.0 && return 2.0
    return sum_sq_diff / sum_sq
end

function ljung_box_q_stat(residuals::Vector{Float64}, lags::Int, task_id::String)
    n = length(residuals)
    n < 4 && return nothing
    h = min(lags, n - 1)
    mean_value = sum(residuals) / n
    variance_sum = sum((value - mean_value)^2 for value in residuals)
    variance_sum <= 0.0 && return nothing

    q = 0.0
    for lag in 1:h
        check_cancelled(task_id)
        covariance_sum = 0.0
        for index in (lag + 1):n
            covariance_sum += (residuals[index] - mean_value) * (residuals[index - lag] - mean_value)
        end
        rho = covariance_sum / variance_sum
        q += rho * rho / (n - lag)
    end
    stat = n * (n + 2.0) * q
    return (stat = stat, p_value = chisq_sf(stat, h), lags = h)
end

function breusch_godfrey_stat(residuals::Vector{Float64}, exog, lags::Int, nomiss0::Bool, task_id::String)
    exog === nothing && return nothing
    n = length(residuals)
    k = size(exog, 2)
    n < 4 && return nothing
    k == 0 && return nothing
    size(exog, 1) == n || return nothing
    p = min(lags, n - 1)

    if nomiss0
        n_aux = n
        z = zeros(Float64, n_aux, p + k)
        y = copy(residuals)
        for t in 1:n
            for lag in 1:p
                z[t, lag] = t > lag ? residuals[t - lag] : 0.0
            end
            z[t, (p + 1):(p + k)] = exog[t, :]
        end
    else
        n_aux = n - p
        n_aux <= k + p && return nothing
        z = zeros(Float64, n_aux, p + k)
        y = Vector{Float64}(undef, n_aux)
        for (row, t) in enumerate((p + 1):n)
            y[row] = residuals[t]
            for lag in 1:p
                z[row, lag] = residuals[t - lag]
            end
            z[row, (p + 1):(p + k)] = exog[t, :]
        end
    end

    check_cancelled(task_id)
    gamma = try
        factor = cholesky(Symmetric(transpose(z) * z))
        factor.L' \ (factor.L \ (transpose(z) * y))
    catch
        return nothing
    end
    y_hat = z * gamma
    rss = sum((actual - fitted)^2 for (actual, fitted) in zip(y, y_hat))
    tss = sum(value * value for value in y)
    r2 = tss > 1e-20 ? 1.0 - rss / tss : 0.0
    stat = n_aux * r2
    return (stat = stat, p_value = chisq_sf(stat, p), lags = p)
end

function run_serial_tests(params, task_id::String)
    input_path = require_string(field(params, "inputPath"), "inputPath")
    output_path = require_string(field(params, "outputPath"), "outputPath")
    metadata_path = require_string(field(params, "metadataPath"), "metadataPath")
    parameters = field(params, "parameters")
    parameters === nothing && throw(ArgumentError("`parameters` is required"))

    residual_column = require_string(field(parameters, "residualColumn"), "parameters.residualColumn")
    exog_columns = optional_string_vector(field(parameters, "exogColumns"), "parameters.exogColumns")
    bg_nomiss0 = parse_bool(field(parameters, "bgNomiss0"), "parameters.bgNomiss0", true)

    table = Arrow.Table(input_path)
    residuals = serial_numeric_column(table, residual_column, task_id)
    lags = parse_lags(field(parameters, "lags"), length(residuals))
    exog = serial_numeric_matrix(table, exog_columns, length(residuals), task_id)

    dw = durbin_watson_stat(residuals)
    q = ljung_box_q_stat(residuals, lags, task_id)
    bg = breusch_godfrey_stat(residuals, exog, lags, bg_nomiss0, task_id)

    check_cancelled(task_id)
    Arrow.write(output_path, (
        dw_d = [dw],
        q_stat = Union{Missing, Float64}[q === nothing ? missing : q.stat],
        q_p_value = Union{Missing, Float64}[q === nothing ? missing : q.p_value],
        q_lags = Union{Missing, Int64}[q === nothing ? missing : q.lags],
        bg_stat = Union{Missing, Float64}[bg === nothing ? missing : bg.stat],
        bg_p_value = Union{Missing, Float64}[bg === nothing ? missing : bg.p_value],
        bg_lags = Union{Missing, Int64}[bg === nothing ? missing : bg.lags],
    ))
    open(metadata_path, "w") do file
        JSON3.write(file, Dict(
            "taskId" => task_id,
            "operation" => "serial_tests",
            "observationCount" => length(residuals),
            "lags" => lags,
            "hasExog" => exog !== nothing,
        ))
    end
    check_cancelled(task_id)
    return Dict(
        "taskId" => task_id,
        "outputPath" => output_path,
        "metadataPath" => metadata_path,
    )
end
