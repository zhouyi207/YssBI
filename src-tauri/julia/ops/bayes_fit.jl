


function run_bayes_fit(params, task_id::String)
    send_progress(task_id, "loading_data")
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

    exchange === nothing && throw(ArgumentError("Bayesian exchange manifest is required"))
    result = bayes_try_turing_generic_normal_fit(
        model, exchange, input_table, input_rows, task_id, output_path, metadata_path,
    )
    result === nothing && throw(UnsupportedBayesCapability(
        "Turing execution supports only Normal, BernoulliLogit, and PoissonLog regression models with supported scalar priors",
    ))


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

bayes_chain_values(chain) = Array(chain.value)

function bayes_sample_with_progress(model_instance, sampler, draws::Int, warmup::Int, chains::Int, task_id::String)
    iterations_per_chain = warmup + draws
    total = chains * iterations_per_chain
    report_interval = max(1, total ÷ 200)

    callback = function (rng, model, sampler_state, transition, state, iteration; chain_number = 1, kwargs...)
        check_cancelled(task_id)
        completed = (Int(chain_number) - 1) * iterations_per_chain + Int(iteration)
        if completed == 1 || completed == total || completed % report_interval == 0 || iteration == warmup || iteration == warmup + 1
            stage = iteration <= warmup ? "warmup" : "sampling"
            send_progress(task_id, stage; completed = completed, total = total)
        end
        return nothing
    end

    chain_with_warmup = with_sampling_error() do
        with_logger(NullLogger()) do
            sample(
                model_instance,
                sampler,
                MCMCSerial(),
                iterations_per_chain,
                chains;
                discard_adapt = false,
                progress = false,
                callback = callback,
            )
        end
    end
    return chain_with_warmup[(warmup + 1):iterations_per_chain, :, :]
end



function bayes_try_turing_generic_normal_fit(model, exchange, table, input_rows::Int, task_id::String, output_path::String, metadata_path::String)
    runner = Base.invokelatest(getfield, Main, :bayes_try_generic_normal_turing)
    return Base.invokelatest(
        runner,
        model,
        exchange,
        table,
        input_rows,
        task_id,
        output_path,
        metadata_path,
    )
end
