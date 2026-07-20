include(joinpath(@__DIR__, "bayes", "expression.jl"))

struct UnsupportedBayesCapability <: Exception
    message::String
end

Base.showerror(io::IO, error::UnsupportedBayesCapability) = print(io, "unsupported capability: ", error.message)

function run_bayes_fit(params, task_id::String)
    input_path = require_string(field(params, "inputPath"), "inputPath")
    output_path = require_string(field(params, "outputPath"), "outputPath")
    metadata_path = require_string(field(params, "metadataPath"), "metadataPath")
    parameters = field(params, "parameters")
    parameters === nothing && throw(ArgumentError("`parameters` is required"))

    exchange = bayes_read_exchange_manifest(params, input_path)
    model = nothing
    if exchange !== nothing
        input_path = require_string(field(exchange, "inputTablePath"), "exchange.inputTablePath")
        output_path = require_string(field(exchange, "outputPath"), "exchange.outputPath")
        metadata_path = require_string(field(exchange, "metadataPath"), "exchange.metadataPath")
        model_spec_path = require_string(field(exchange, "modelSpecPath"), "exchange.modelSpecPath")
        model = JSON3.read(read(model_spec_path, String))
    else
        model = field(parameters, "model")
    end
    model === nothing && throw(ArgumentError("Bayesian model spec is required"))

    check_cancelled(task_id)

    input_table = Arrow.Table(input_path)
    input_columns = String.(propertynames(input_table))
    input_rows = isempty(input_columns) ? 0 : length(getproperty(input_table, Symbol(input_columns[1])))

    predictor_preview = bayes_predictor_preview(model, input_table, input_rows, task_id)
    preview_text = isempty(predictor_preview) ? "no preview rows" : join(string.(predictor_preview), ", ")

    base_warnings = Any[
        Dict(
            "code" => "JULIA_BAYES_ENGINE_READY",
            "message" => "Julia Bayesian engine op is reachable.",
            "parameter" => nothing,
        ),
        Dict(
            "code" => "JULIA_BAYES_INPUT_READY",
            "message" => "Julia received $(input_rows) rows and $(length(input_columns)) columns: $(join(input_columns, ", ")).",
            "parameter" => nothing,
        ),
        Dict(
            "code" => "JULIA_BAYES_PREDICTOR_READY",
            "message" => "Predictor AST evaluated successfully for preview values: $(preview_text).",
            "parameter" => nothing,
        ),
    ]

    result = bayes_try_turing_linear_fit(model, input_table, input_rows, task_id, output_path, metadata_path)
    if result === nothing
        result = bayes_try_turing_generic_normal_fit(model, input_table, input_rows, task_id, output_path, metadata_path)
    end
    result === nothing && throw(UnsupportedBayesCapability(
        "Turing execution supports only Normal, BernoulliLogit, and PoissonLog regression models with supported scalar priors",
    ))
    append!(result["diagnostics"]["warnings"], base_warnings)

    summary_path = joinpath(dirname(metadata_path), "summary.json")
    bayes_attach_artifact_manifest!(result, task_id, summary_path, metadata_path)
    open(summary_path, "w") do file
        JSON3.write(file, Dict(
            "summaries" => result["summaries"],
            "diagnostics" => result["diagnostics"],
        ))
    end
    open(metadata_path, "w") do file
        JSON3.write(file, result)
    end

    check_cancelled(task_id)
    return Dict(
        "taskId" => task_id,
        "operation" => "bayes_fit",
        "outputPath" => output_path,
        "metadataPath" => metadata_path,
    )
end

function bayes_read_exchange_manifest(params, input_path::String)
    parameters = field(params, "parameters", nothing)
    explicit_path = parameters === nothing ? nothing : field(parameters, "exchangeManifestPath", nothing)
    manifest_path = explicit_path === nothing ? joinpath(dirname(input_path), "exchange_manifest.json") : String(explicit_path)
    isfile(manifest_path) || return nothing
    return JSON3.read(read(manifest_path, String))
end

function bayes_attach_artifact_manifest!(result, task_id::String, summary_path::String, metadata_path::String)
    artifacts = pop!(result, "artifacts", Any[])
    prepend!(artifacts, Any[
        Dict(
            "kind" => "summary",
            "format" => "json",
            "path" => summary_path,
            "rows" => length(get(result, "summaries", Any[])),
        ),
        Dict(
            "kind" => "metadata",
            "format" => "json",
            "path" => metadata_path,
            "rows" => nothing,
        ),
    ])


    result["artifactManifest"] = Dict(
        "taskId" => task_id,
        "artifacts" => artifacts,
    )
    return result
end

function bayes_try_turing_linear_fit(model, table, input_rows::Int, task_id::String, output_path::String, metadata_path::String)
    if !isdefined(Main, :bayes_run_fixed_linear_turing)
        include(joinpath(@__DIR__, "bayes", "turing_linear.jl"))
    end
    return Base.invokelatest(
        bayes_run_fixed_linear_turing,
        model,
        table,
        input_rows,
        task_id,
        output_path,
        metadata_path,
    )
end

function bayes_try_turing_generic_normal_fit(model, table, input_rows::Int, task_id::String, output_path::String, metadata_path::String)
    if !isdefined(Main, :bayes_try_generic_normal_turing)
        include(joinpath(@__DIR__, "bayes", "turing_generic_normal.jl"))
    end
    return Base.invokelatest(
        bayes_try_generic_normal_turing,
        model,
        table,
        input_rows,
        task_id,
        output_path,
        metadata_path,
    )
end
