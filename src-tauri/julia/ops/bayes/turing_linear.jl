using Arrow
using Distributions
using Logging
using MCMCChains
using Random
using Statistics
using Turing

@model function yssbi_linear_normal_model(x, y, a_prior, b_prior, sigma_prior)
    a ~ a_prior
    b ~ b_prior
    sigma ~ sigma_prior

    for index in eachindex(y)
        y[index] ~ Normal(a * x[index] + b, sigma)
    end
end

function bayes_numeric_vector(table, column_name::String, task_id::String)
    column_symbol = Symbol(column_name)
    hasproperty(table, column_symbol) || throw(ArgumentError("column `$column_name` was not found"))
    column = getproperty(table, column_symbol)
    values = Vector{Float64}(undef, length(column))
    for index in eachindex(column)
        check_cancelled(task_id)
        value = column[index]
        value isa Real && !(value isa Bool) || throw(ArgumentError("column `$column_name` must contain numeric values"))
        numeric_value = Float64(value)
        isfinite(numeric_value) || throw(ArgumentError("column `$column_name` must contain finite values"))
        values[index] = numeric_value
    end
    return values
end

function bayes_parameter_spec(model, name::String)
    for parameter in field(model, "parameters", Any[])
        field(parameter, "name", "") == name && return parameter
    end
    throw(ArgumentError("parameter `$name` was not found"))
end

function bayes_distribution_from_prior(prior, role::String)
    distribution = String(field(prior, "distribution", ""))
    args = field(prior, "args", Any[])

    if distribution == "normal"
        return Normal(Float64(args[1]), Float64(args[2]))
    elseif distribution == "log_normal"
        return LogNormal(Float64(args[1]), Float64(args[2]))
    elseif distribution == "uniform"
        return Uniform(Float64(args[1]), Float64(args[2]))
    elseif distribution == "beta"
        return Beta(Float64(args[1]), Float64(args[2]))
    elseif distribution == "gamma"
        return Gamma(Float64(args[1]), Float64(args[2]))
    elseif distribution == "exponential"
        return Exponential(1.0 / Float64(args[1]))
    elseif distribution == "student_t"
        return LocationScale(Float64(args[2]), Float64(args[3]), TDist(Float64(args[1])))
    elseif distribution == "cauchy"
        return Cauchy(Float64(args[1]), Float64(args[2]))
    elseif distribution == "half_normal"
        return truncated(Normal(0.0, Float64(args[1])); lower = 0.0)
    end

    throw(UnsupportedBayesCapability("$role prior `$distribution` is not supported by the Julia Bayesian engine"))
end

function bayes_fixed_linear_parts(expr)
    field(expr, "type", "") == "binary" || return nothing
    field(expr, "op", "") == "add" || return nothing
    left = field(expr, "left")
    right = field(expr, "right")

    product = bayes_parameter_times_data_variable(left)
    intercept = bayes_parameter_name(right)
    if product !== nothing && intercept !== nothing
        return (slope = product.parameter, variable = product.variable, intercept = intercept)
    end

    product = bayes_parameter_times_data_variable(right)
    intercept = bayes_parameter_name(left)
    if product !== nothing && intercept !== nothing
        return (slope = product.parameter, variable = product.variable, intercept = intercept)
    end

    return nothing
end

function bayes_parameter_times_data_variable(expr)
    field(expr, "type", "") == "binary" || return nothing
    field(expr, "op", "") == "mul" || return nothing
    left = field(expr, "left")
    right = field(expr, "right")

    parameter = bayes_parameter_name(left)
    variable = bayes_data_variable_name(right)
    parameter !== nothing && variable !== nothing && return (parameter = parameter, variable = variable)

    parameter = bayes_parameter_name(right)
    variable = bayes_data_variable_name(left)
    parameter !== nothing && variable !== nothing && return (parameter = parameter, variable = variable)

    return nothing
end

function bayes_parameter_name(expr)
    field(expr, "type", "") == "parameter" || return nothing
    return require_string(field(expr, "name"), "expression.name")
end

function bayes_data_variable_name(expr)
    field(expr, "type", "") == "data_variable" || return nothing
    return require_string(field(expr, "name"), "expression.name")
end

function bayes_normal_sigma_parameter(model)
    likelihood = field(model, "likelihood")
    field(likelihood, "type", "") == "normal" || return nothing
    sigma = field(likelihood, "sigma", nothing)
    sigma === nothing && return nothing
    return require_string(field(sigma, "parameter"), "likelihood.sigma.parameter")
end

function bayes_run_fixed_linear_turing(model, table, input_rows::Int, task_id::String, output_path::String, metadata_path::String)
    input_rows >= 4 || throw(ArgumentError("fixed linear Turing PoC requires at least 4 observations"))
    parts = bayes_fixed_linear_parts(field(model, "predictor"))
    parts === nothing && return nothing

    data_variables = field(model, "dataVariables", nothing)
    data_variables === nothing && throw(ArgumentError("model dataVariables are required"))
    x_column = require_string(field(data_variables, parts.variable), "dataVariables.$(parts.variable)")
    response = field(model, "response")
    sigma_parameter = bayes_normal_sigma_parameter(model)
    sigma_parameter === nothing && return nothing

    x = bayes_numeric_vector(table, x_column, task_id)
    y = bayes_response_vector(table, response, "normal", task_id)
    length(x) == length(y) || throw(ArgumentError("response and predictor columns must have the same length"))

    a_prior = bayes_distribution_from_prior(field(bayes_parameter_spec(model, parts.slope), "prior"), "slope")
    b_prior = bayes_distribution_from_prior(field(bayes_parameter_spec(model, parts.intercept), "prior"), "intercept")
    sigma_prior = bayes_distribution_from_prior(field(bayes_parameter_spec(model, sigma_parameter), "prior"), "sigma")

    sampler = field(model, "sampler", nothing)
    draws = Int(field(sampler, "samples", 1_000))
    warmup = Int(field(sampler, "warmup", 500))
    chains = Int(field(sampler, "chains", 1))
    target_accept = Float64(field(sampler, "targetAccept", 0.8))
    max_tree_depth = Int(field(sampler, "maxTreeDepth", 10))
    seed = field(sampler, "seed", nothing)
    seed !== nothing && Random.seed!(UInt(seed))

    model_instance = yssbi_linear_normal_model(x, y, a_prior, b_prior, sigma_prior)
    chain = bayes_sample_with_progress(
        model_instance,
        bayes_nuts_sampler(warmup, target_accept, max_tree_depth),
        draws,
        warmup,
        chains,
        task_id,
    )
    model_parameter_names = [parts.slope, parts.intercept, sigma_parameter]
    send_progress(task_id, "summarizing")
    summaries = bayes_chain_summaries(chain, ["a", "b", "sigma"], model_parameter_names)
    artifacts = Any[]
    if Bool(field(sampler, "saveSamples", false))
        send_progress(task_id, "writing_samples")
        bayes_write_samples(output_path, chain, ["a", "b", "sigma"], model_parameter_names)
        push!(artifacts, Dict(
            "kind" => "posterior_samples",
            "format" => "arrow_ipc",
            "path" => output_path,
            "rows" => nothing,
        ))
    end
    ppc_path = joinpath(dirname(output_path), "posterior_predictive.arrow")
    send_progress(task_id, "posterior_predictive")
    bayes_write_posterior_predictive(ppc_path, chain, x, y, response, task_id)
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

function bayes_nuts_sampler(warmup::Int, target_accept::Float64, max_tree_depth::Int)
    return NUTS(warmup, target_accept; max_depth = max_tree_depth)
end

function bayes_max_treedepth_hits(chain, max_tree_depth::Int)
    haskey(getfield(chain, :name_map), :internals) || return nothing
    internal_names = names(chain, :internals)
    tree_depth_index = findfirst(name -> String(name) in ("tree_depth", "tree_depth__"), internal_names)
    tree_depth_index === nothing && return nothing
    internals = MCMCChains.get_sections(chain, :internals)
    values = bayes_chain_values(internals[:, [internal_names[tree_depth_index]], :])
    return count(value -> Int(value) >= max_tree_depth, values)
end

function bayes_write_posterior_predictive(path::String, chain, x::Vector{Float64}, y::Vector{Float64}, response, task_id::String)
    values = bayes_chain_values(chain)
    available_names = String.(names(chain))
    a_index = findfirst(name -> name == "a", available_names)
    b_index = findfirst(name -> name == "b", available_names)
    sigma_index = findfirst(name -> name == "sigma", available_names)
    a_index === nothing && throw(ArgumentError("chain parameter `a` was not found"))
    b_index === nothing && throw(ArgumentError("chain parameter `b` was not found"))
    sigma_index === nothing && throw(ArgumentError("chain parameter `sigma` was not found"))

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
                a = Float64(values[draw_index, a_index, chain_index])
                b = Float64(values[draw_index, b_index, chain_index])
                sigma = abs(Float64(values[draw_index, sigma_index, chain_index]))
                mu = a * x[observation] + b
                predictions[prediction_index] = rand(Normal(mu, sigma))
                prediction_index += 1
            end
        end
        summaries = bayes_predictive_scale_summaries(response, predictions)
        push!(observations, Int(observation))
        push!(response_transforms, bayes_response_transform(response))
        push!(observed_model, y[observation])
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

function bayes_write_samples(output_path::String, chain, chain_names::Vector{String}, model_names::Vector{String})
    values = bayes_chain_values(chain)
    available_names = String.(names(chain))
    parameters = String[]
    chains = Int[]
    draws = Int[]
    sample_values = Float64[]
    row_capacity = length(chain_names) * size(values, 1) * size(values, 3)
    sizehint!(parameters, row_capacity)
    sizehint!(chains, row_capacity)
    sizehint!(draws, row_capacity)
    sizehint!(sample_values, row_capacity)

    for (chain_name, model_name) in zip(chain_names, model_names)
        parameter_index = findfirst(name -> name == chain_name, available_names)
        parameter_index === nothing && continue
        for chain_index in axes(values, 3)
            for draw_index in axes(values, 1)
                value = Float64(values[draw_index, parameter_index, chain_index])
                isfinite(value) || continue
                push!(parameters, model_name)
                push!(chains, Int(chain_index))
                push!(draws, Int(draw_index))
                push!(sample_values, value)
            end
        end
    end

    Arrow.write(output_path, (
        parameter = parameters,
        chain = chains,
        draw = draws,
        value = sample_values,
    ))
end

function bayes_chain_summaries(chain, chain_names::Vector{String}, model_names::Vector{String})
    values = bayes_chain_values(chain)
    available_names = String.(names(chain))
    stats = summarystats(chain).nt
    stats_names = String.(stats.parameters)
    summaries = Any[]
    sizehint!(summaries, length(chain_names))

    for (chain_name, model_name) in zip(chain_names, model_names)
        draw_index = findfirst(name -> name == chain_name, available_names)
        stats_index = findfirst(name -> name == chain_name, stats_names)
        draw_index === nothing && continue
        stats_index === nothing && continue

        draws = vec(values[:, draw_index, :])
        isempty(draws) && continue
        sort!(draws)
        q025, median, q975 = quantile(draws, [0.025, 0.5, 0.975]; sorted = true)
        push!(summaries, Dict(
            "parameter" => model_name,
            "mean" => finite_or_nothing(stats.mean[stats_index]),
            "sd" => finite_or_nothing(stats.std[stats_index]),
            "median" => median,
            "q025" => q025,
            "q975" => q975,
            "rhat" => finite_or_nothing(stats.rhat[stats_index]),
            "essBulk" => finite_or_nothing(stats.ess_bulk[stats_index]),
            "essTail" => finite_or_nothing(stats.ess_tail[stats_index]),
        ))
    end

    return summaries
end

function finite_or_nothing(value)
    value isa Real || return nothing
    numeric = Float64(value)
    return isfinite(numeric) ? numeric : nothing
end

function bayes_diagnostic_warnings(summaries, total_draws::Int)
    warnings = Any[]
    ess_threshold = min(100.0, max(10.0, 0.1 * Float64(total_draws)))
    for summary in summaries
        parameter = String(summary["parameter"])
        rhat_value = summary["rhat"]
        if rhat_value !== nothing && rhat_value > 1.01
            push!(warnings, Dict(
                "code" => "RHAT_TOO_HIGH",
                "message" => "R-hat is $(round(rhat_value; digits = 4)); consider increasing samples or checking model geometry.",
                "parameter" => parameter,
            ))
        end

        ess_bulk = summary["essBulk"]
        if ess_bulk !== nothing && ess_bulk < ess_threshold
            push!(warnings, Dict(
                "code" => "ESS_TOO_LOW",
                "message" => "Bulk ESS is $(round(ess_bulk; digits = 2)); consider increasing samples.",
                "parameter" => parameter,
            ))
        end

        ess_tail = summary["essTail"]
        if ess_tail !== nothing && ess_tail < ess_threshold
            push!(warnings, Dict(
                "code" => "ESS_TOO_LOW",
                "message" => "Tail ESS is $(round(ess_tail; digits = 2)); credible interval estimates may be unstable.",
                "parameter" => parameter,
            ))
        end
    end
    return warnings
end
