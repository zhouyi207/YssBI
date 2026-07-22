

@model function yssbi_generated_regression_model(columns, likelihood, y, priors)
    theta ~ arraydist(priors)
    Turing.@addlogprob! likelihood(theta, columns, y)
end

function bayes_load_predictor(exchange, table, row_count::Int, task_id::String)
    kernel_path = require_string(field(exchange, "predictorKernelPath"), "exchange.predictorKernelPath")
    predictor_expression = Meta.parse(read(kernel_path, String))
    predictor = @RuntimeGeneratedFunction(predictor_expression)
    likelihood_path = require_string(
        field(exchange, "likelihoodKernelPath"), "exchange.likelihoodKernelPath",
    )
    likelihood_expression = Meta.parse(read(likelihood_path, String))
    likelihood = @RuntimeGeneratedFunction(likelihood_expression)

    column_names = String.(field(exchange, "predictorColumns", String[]))
    columns = Matrix{Float64}(undef, row_count, length(column_names))
    for (column_index, column_name) in enumerate(column_names)
        values = bayes_numeric_vector(table, column_name, task_id)
        length(values) == row_count || throw(ArgumentError("predictor column `$column_name` has an invalid length"))
        columns[:, column_index] = values
    end
    return (predictor = predictor, likelihood = likelihood, columns = columns)
end



function bayes_try_generic_normal_turing(model, exchange, table, input_rows::Int, task_id::String, output_path::String, metadata_path::String)
    input_rows >= 4 || throw(ArgumentError("generic Turing regression requires at least 4 observations"))
    likelihood = field(model, "likelihood")
    likelihood_type = String(field(likelihood, "type", ""))
    likelihood_type in ("normal", "bernoulli_logit", "poisson_log") || return nothing

    send_progress(task_id, "preparing_response")
    response = field(model, "response")
    y = bayes_response_vector(table, response, likelihood_type, task_id)
    length(y) == input_rows || throw(ArgumentError("response column length must match input rows"))

    send_progress(task_id, "loading_kernels")
    compiled_predictor = bayes_load_predictor(exchange, table, input_rows, task_id)
    send_progress(task_id, "preparing_kernels")
    preview_parameters = [
        bayes_prior_default(field(parameter, "prior"))
        for parameter in field(model, "parameters", Any[])
    ]
    for row_index in 1:min(input_rows, 5)
        value = compiled_predictor.predictor(preview_parameters, compiled_predictor.columns, row_index)
        isfinite(value) || throw(ArgumentError("predictor returned a non-finite value at row $row_index"))
    end


    send_progress(task_id, "building_model")
    sigma_parameter = likelihood_type == "normal" ? bayes_normal_sigma_parameter(model) : nothing
    likelihood_type == "normal" && sigma_parameter === nothing && return nothing

    parameter_specs = field(model, "parameters", Any[])
    parameter_names = String[]
    priors = Distribution{Univariate, Continuous}[]
    sigma_index = nothing
    for parameter in parameter_specs
        name = require_string(field(parameter, "name"), "parameter.name")
        prior = bayes_distribution_from_prior(field(parameter, "prior"), "parameter `$name`")
        push!(parameter_names, name)
        push!(priors, prior)
        if sigma_parameter !== nothing && name == sigma_parameter
            sigma_index = length(parameter_names)
        end
    end
    likelihood_type == "normal" && sigma_index === nothing && throw(ArgumentError("sigma parameter `$sigma_parameter` was not found"))

    sampler = field(model, "sampler", nothing)
    draws = Int(field(sampler, "samples", 1_000))
    warmup = Int(field(sampler, "warmup", 500))
    chains = Int(field(sampler, "chains", 1))
    target_accept = Float64(field(sampler, "targetAccept", 0.8))
    max_tree_depth = Int(field(sampler, "maxTreeDepth", 10))
    seed = field(sampler, "seed", nothing)
    seed !== nothing && Random.seed!(UInt(seed))

    model_instance = yssbi_generated_regression_model(
        compiled_predictor.columns,
        compiled_predictor.likelihood,
        y,
        priors,
    )
    send_progress(task_id, "initializing_nuts")
    chain = bayes_sample_with_progress(
        model_instance,
        bayes_nuts_sampler(warmup, target_accept, max_tree_depth),
        draws,
        warmup,
        chains,
        task_id,
    )

    chain_names = ["theta[$index]" for index in eachindex(parameter_names)]
    send_progress(task_id, "summarizing")
    summaries = bayes_chain_summaries(chain, chain_names, parameter_names)

    artifacts = Any[]
    if Bool(field(sampler, "saveSamples", false))
        send_progress(task_id, "writing_samples")
        bayes_write_samples(output_path, chain, chain_names, parameter_names)
        push!(artifacts, Dict(
            "kind" => "posterior_samples",
            "format" => "arrow_ipc",
            "path" => output_path,
            "rows" => nothing,
        ))
    end

    ppc_path = joinpath(dirname(output_path), "posterior_predictive.arrow")
    send_progress(task_id, "posterior_predictive")
    bayes_write_generic_posterior_predictive(
        ppc_path, chain, chain_names, parameter_names, compiled_predictor,
        y, likelihood_type, sigma_parameter, response, task_id,
    )
    push!(artifacts, Dict(
        "kind" => "posterior_predictive",
        "format" => "arrow_ipc",
        "path" => ppc_path,
        "rows" => nothing,
    ))

    warnings = Any[]

    append!(warnings, bayes_diagnostic_warnings(summaries, draws * chains))
    send_progress(task_id, "finalizing")

    return Dict(
        "summaries" => summaries,
        "diagnostics" => Dict(
            "chains" => chains,
            "drawsPerChain" => draws,
            "warmup" => warmup,
            "divergences" => nothing,
            "maxTreedepthHits" => bayes_max_treedepth_hits(chain, max_tree_depth),
            "warnings" => warnings,
        ),
        "artifacts" => artifacts,
    )
end



function bayes_write_generic_posterior_predictive(path::String, chain, chain_names::Vector{String}, model_names::Vector{String}, compiled_predictor, y, likelihood_type::String, sigma_parameter, response, task_id::String)
    values = bayes_chain_values(chain)
    available_names = String.(names(chain))
    parameter_indices = Dict{String, Int}()
    chain_parameter_indices = Int[]
    for (chain_name, model_name) in zip(chain_names, model_names)
        parameter_index = findfirst(name -> name == chain_name, available_names)
        parameter_index === nothing && throw(ArgumentError("chain parameter `$chain_name` was not found"))
        parameter_indices[model_name] = parameter_index
        push!(chain_parameter_indices, parameter_index)
    end
    likelihood_type == "normal" && !haskey(parameter_indices, sigma_parameter) && throw(ArgumentError("chain sigma parameter `$sigma_parameter` was not found"))

    observations = Int[]
    response_transforms = String[]
    observed_model = Float64[]
    mean_model = Float64[]
    q025_model = Float64[]
    q975_model = Float64[]
    observed_original = Float64[]
    mean_original = Float64[]
    q025_original = Float64[]
    q975_original = Float64[]

    prediction_count = size(values, 1) * size(values, 3)
    for observation in eachindex(y)
        check_cancelled(task_id)
        predictions = Vector{Float64}(undef, prediction_count)
        prediction_index = 1
        for chain_index in axes(values, 3)
            for draw_index in axes(values, 1)
                draw_index % 256 == 1 && check_cancelled(task_id)
                theta = @view values[draw_index, chain_parameter_indices, chain_index]
                linear_predictor = compiled_predictor.predictor(
                    theta, compiled_predictor.columns, observation,
                )
                prediction = if likelihood_type == "normal"
                    sigma = abs(Float64(values[
                        draw_index, parameter_indices[sigma_parameter], chain_index,
                    ]))
                    rand(Normal(Float64(linear_predictor), sigma))
                elseif likelihood_type == "bernoulli_logit"
                    Float64(rand(Bernoulli(bayes_logistic(linear_predictor))))
                elseif likelihood_type == "poisson_log"
                    Float64(rand(Poisson(exp(Float64(linear_predictor)))))
                else
                    throw(ArgumentError("unsupported posterior predictive likelihood `$likelihood_type`"))
                end
                predictions[prediction_index] = prediction
                prediction_index += 1
            end
        end
        summaries = bayes_predictive_scale_summaries(response, predictions)
        push!(observations, Int(observation))
        push!(response_transforms, bayes_response_transform(response))
        push!(observed_model, Float64(y[observation]))
        push!(mean_model, summaries.model_mean)
        push!(q025_model, summaries.model_q025)
        push!(q975_model, summaries.model_q975)
        push!(observed_original, bayes_inverse_response(response, y[observation]))
        push!(mean_original, summaries.original_mean)
        push!(q025_original, summaries.original_q025)
        push!(q975_original, summaries.original_q975)
    end

    Arrow.write(path, (
        observation = observations,
        response_transform = response_transforms,
        observed_model = observed_model,
        mean_model = mean_model,
        q025_model = q025_model,
        q975_model = q975_model,
        observed_original = observed_original,
        mean_original = mean_original,
        q025_original = q025_original,
        q975_original = q975_original,
    ))
end

function bayes_logistic(value)
    if value >= zero(value)
        z = exp(-value)
        return one(value) / (one(value) + z)
    end
    z = exp(value)
    return z / (one(value) + z)
end
