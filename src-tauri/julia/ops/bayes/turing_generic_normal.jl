using Arrow
using Distributions
using Logging
using MCMCChains
using Random
using Statistics
using Turing

@model function yssbi_compiled_regression_model(design, offset, y, likelihood_type::String, priors, sigma_index)
    theta ~ arraydist(priors)

    for row_index in eachindex(y)
        linear_predictor = offset[row_index]
        for parameter_index in axes(design, 2)
            linear_predictor += design[row_index, parameter_index] * theta[parameter_index]
        end

        if likelihood_type == "normal"
            y[row_index] ~ Normal(linear_predictor, theta[Int(sigma_index)])
        elseif likelihood_type == "bernoulli_logit"
            y[row_index] ~ Bernoulli(bayes_logistic(linear_predictor))
        elseif likelihood_type == "poisson_log"
            y[row_index] ~ Poisson(exp(linear_predictor))
        else
            throw(ArgumentError("unsupported compiled Turing likelihood `$likelihood_type`"))
        end
    end
end

@model function yssbi_generic_regression_model(table, data_variables, predictor, y, likelihood_type::String, parameter_names, priors, sigma_index, task_id::String)
    theta ~ arraydist(priors)

    for row_index in eachindex(y)
        params = Dict{String, Any}()
        for index in eachindex(parameter_names)
            params[parameter_names[index]] = theta[index]
        end
        linear_predictor = bayes_evaluate_expression(predictor, table, data_variables, params, row_index, task_id)

        if likelihood_type == "normal"
            sigma = theta[Int(sigma_index)]
            y[row_index] ~ Normal(linear_predictor, sigma)
        elseif likelihood_type == "bernoulli_logit"
            y[row_index] ~ Bernoulli(bayes_logistic(linear_predictor))
        elseif likelihood_type == "poisson_log"
            y[row_index] ~ Poisson(exp(linear_predictor))
        else
            throw(ArgumentError("unsupported generic Turing likelihood `$likelihood_type`"))
        end
    end
end

function bayes_expression_depends_on_parameters(expr)::Bool
    node_type = String(field(expr, "type", ""))
    if node_type == "parameter"
        return true
    elseif node_type in ("number", "data_variable", "column")
        return false
    elseif node_type == "unary"
        return bayes_expression_depends_on_parameters(field(expr, "arg"))
    elseif node_type == "binary"
        return bayes_expression_depends_on_parameters(field(expr, "left")) ||
            bayes_expression_depends_on_parameters(field(expr, "right"))
    elseif node_type == "call"
        return any(bayes_expression_depends_on_parameters, field(expr, "args", Any[]))
    end
    return true
end

function bayes_expression_is_affine(expr)::Bool
    node_type = String(field(expr, "type", ""))
    if node_type in ("number", "data_variable", "column", "parameter")
        return true
    elseif node_type == "unary"
        return String(field(expr, "op", "")) == "neg" &&
            bayes_expression_is_affine(field(expr, "arg"))
    elseif node_type == "binary"
        op = String(field(expr, "op", ""))
        left = field(expr, "left")
        right = field(expr, "right")
        if op in ("add", "sub")
            return bayes_expression_is_affine(left) && bayes_expression_is_affine(right)
        elseif op == "mul"
            return bayes_expression_is_affine(left) && bayes_expression_is_affine(right) &&
                !(bayes_expression_depends_on_parameters(left) && bayes_expression_depends_on_parameters(right))
        elseif op == "div"
            return bayes_expression_is_affine(left) && !bayes_expression_depends_on_parameters(right)
        end
        return !bayes_expression_depends_on_parameters(expr)
    elseif node_type == "call"
        return !bayes_expression_depends_on_parameters(expr)
    end
    return false
end

function bayes_compile_affine_predictor(predictor, table, data_variables, parameter_names::Vector{String}, row_count::Int, task_id::String)
    bayes_expression_is_affine(predictor) || return nothing
    parameters = Dict{String, Any}(name => 0.0 for name in parameter_names)
    offset = Vector{Float64}(undef, row_count)
    design = Matrix{Float64}(undef, row_count, length(parameter_names))

    for row_index in 1:row_count
        row_index % 256 == 1 && check_cancelled(task_id)
        offset[row_index] = Float64(bayes_evaluate_expression(
            predictor, table, data_variables, parameters, row_index, task_id,
        ))
    end
    for (parameter_index, parameter_name) in enumerate(parameter_names)
        parameters[parameter_name] = 1.0
        for row_index in 1:row_count
            row_index % 256 == 1 && check_cancelled(task_id)
            value = bayes_evaluate_expression(
                predictor, table, data_variables, parameters, row_index, task_id,
            )
            design[row_index, parameter_index] = Float64(value) - offset[row_index]
        end
        parameters[parameter_name] = 0.0
    end
    return (design = design, offset = offset)
end

function bayes_try_generic_normal_turing(model, table, input_rows::Int, task_id::String, output_path::String, metadata_path::String)
    input_rows >= 4 || throw(ArgumentError("generic Turing regression requires at least 4 observations"))
    likelihood = field(model, "likelihood")
    likelihood_type = String(field(likelihood, "type", ""))
    likelihood_type in ("normal", "bernoulli_logit", "poisson_log") || return nothing

    response = field(model, "response")
    y = bayes_response_vector(table, response, likelihood_type, task_id)
    length(y) == input_rows || throw(ArgumentError("response column length must match input rows"))

    data_variables = field(model, "dataVariables", nothing)
    data_variables === nothing && throw(ArgumentError("model dataVariables are required"))
    predictor = field(model, "predictor", nothing)
    predictor === nothing && throw(ArgumentError("model predictor is required"))

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

    compiled_predictor = bayes_compile_affine_predictor(
        predictor,
        table,
        data_variables,
        parameter_names,
        input_rows,
        task_id,
    )
    model_instance = if compiled_predictor === nothing
        yssbi_generic_regression_model(
            table, data_variables, predictor, y, likelihood_type,
            parameter_names, priors, sigma_index, task_id,
        )
    else
        yssbi_compiled_regression_model(
            compiled_predictor.design, compiled_predictor.offset,
            y, likelihood_type, priors, sigma_index,
        )
    end
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
    bayes_write_generic_posterior_predictive(ppc_path, chain, chain_names, parameter_names, table, data_variables, predictor, compiled_predictor, y, likelihood_type, sigma_parameter, response, task_id)
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



function bayes_write_generic_posterior_predictive(path::String, chain, chain_names::Vector{String}, model_names::Vector{String}, table, data_variables, predictor, compiled_predictor, y, likelihood_type::String, sigma_parameter, response, task_id::String)
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
                params = nothing
                linear_predictor = if compiled_predictor === nothing
                    params = Dict{String, Any}()
                    for model_name in model_names
                        params[model_name] = values[draw_index, parameter_indices[model_name], chain_index]
                    end
                    bayes_evaluate_expression(predictor, table, data_variables, params, observation, task_id)
                else
                    value = compiled_predictor.offset[observation]
                    for model_index in eachindex(model_names)
                        value += compiled_predictor.design[observation, model_index] *
                            values[draw_index, chain_parameter_indices[model_index], chain_index]
                    end
                    value
                end
                prediction = if likelihood_type == "normal"
                    sigma = if compiled_predictor === nothing
                        abs(Float64(params[sigma_parameter]))
                    else
                        abs(Float64(values[
                            draw_index, parameter_indices[sigma_parameter], chain_index,
                        ]))
                    end
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
