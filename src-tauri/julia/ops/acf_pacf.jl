function parse_max_lag(value, observation_count::Int)
    value isa Integer || throw(ArgumentError("`parameters.maxLag` must be an integer"))
    max_lag = try
        Int(value)
    catch
        throw(ArgumentError("`parameters.maxLag` is outside the supported range"))
    end
    0 <= max_lag <= observation_count - 1 || throw(ArgumentError(
        "`parameters.maxLag` must be between 0 and $(observation_count - 1)"
    ))
    return max_lag
end

function numeric_column(table, column_name::String, task_id::String)
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
        numeric_value = try
            Float64(value)
        catch
            throw(ArgumentError("column `$column_name` must contain Float64-compatible values"))
        end
        isfinite(numeric_value) || throw(ArgumentError(
            "column `$column_name` must contain only finite values"
        ))
        values[index] = numeric_value
    end

    length(values) >= 4 || throw(ArgumentError("column `$column_name` must contain at least 4 observations"))
    return values
end

function acf_pacf(values::Vector{Float64}, max_lag::Int, task_id::String)
    count = length(values)
    mean_value = sum(values) / count
    centered = values .- mean_value
    autocovariances = Vector{Float64}(undef, max_lag + 1)

    for lag in 0:max_lag
        check_cancelled(task_id)
        total = 0.0
        for index in (lag + 1):count
            total += centered[index] * centered[index - lag]
            index % 256 == 0 && check_cancelled(task_id)
        end
        autocovariances[lag + 1] = total / count
    end

    variance = autocovariances[1]
    variance > 0.0 || throw(ArgumentError("column must have non-zero variance"))
    acf_values = autocovariances ./ variance
    pacf_values = Vector{Union{Missing, Float64}}(undef, max_lag + 1)
    pacf_values[1] = missing
    coefficients = Float64[]

    for lag in 1:max_lag
        check_cancelled(task_id)
        numerator = acf_values[lag + 1]
        denominator = 1.0
        for index in 1:(lag - 1)
            numerator -= coefficients[index] * acf_values[lag - index + 1]
            denominator -= coefficients[index] * acf_values[index + 1]
        end
        abs(denominator) > sqrt(eps(Float64)) || throw(ArgumentError(
            "PACF is numerically undefined at lag $lag"
        ))
        coefficient = numerator / denominator
        updated = Vector{Float64}(undef, lag)
        for index in 1:(lag - 1)
            updated[index] = coefficients[index] - coefficient * coefficients[lag - index]
        end
        updated[lag] = coefficient
        coefficients = updated
        pacf_values[lag + 1] = coefficient
    end

    return acf_values, pacf_values
end

function run_acf_pacf(params, task_id::String)
    input_path = require_string(field(params, "inputPath"), "inputPath")
    output_path = require_string(field(params, "outputPath"), "outputPath")
    metadata_path = require_string(field(params, "metadataPath"), "metadataPath")
    parameters = field(params, "parameters")
    parameters === nothing && throw(ArgumentError("`parameters` is required"))
    column_name = require_string(field(parameters, "column"), "parameters.column")

    table = Arrow.Table(input_path)
    values = numeric_column(table, column_name, task_id)
    max_lag = parse_max_lag(field(parameters, "maxLag"), length(values))
    acf_values, pacf_values = acf_pacf(values, max_lag, task_id)

    check_cancelled(task_id)
    Arrow.write(output_path, (
        lag = collect(0:max_lag),
        acf = acf_values,
        pacf = pacf_values,
    ))
    open(metadata_path, "w") do file
        JSON3.write(file, Dict(
            "taskId" => task_id,
            "operation" => "acf_pacf",
            "observationCount" => length(values),
            "maxLag" => max_lag,
        ))
    end
    check_cancelled(task_id)
    return Dict(
        "taskId" => task_id,
        "outputPath" => output_path,
        "metadataPath" => metadata_path,
    )
end
