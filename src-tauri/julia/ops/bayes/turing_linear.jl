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
    summaries = bayes_chain_summaries(chain, ["a", "b", "sigma"], model_parameter_names)
    artifacts = Any[]
    if Bool(field(sampler, "saveSamples", false))
        bayes_write_samples(output_path, chain, ["a", "b", "sigma"], model_parameter_names)
        push!(artifacts, Dict(
            "kind" => "posterior_samples",
            "format" => "arrow_ipc",
            "path" => output_path,
            "rows" => nothing,
        ))
    end
    ppc_path = joinpath(dirname(output_path), "posterior_predictive.arrow")
    bayes_write_posterior_predictive(ppc_path, chain, x, y)
    push!(artifacts, Dict(
        "kind" => "posterior_predictive",
        "format" => "arrow_ipc",
        "path" => ppc_path,
        "rows" => nothing,
    ))
    warnings = Any[]
    if bayes_response_is_transformed(response)
        push!(warnings, Dict(
            "code" => "JULIA_BAYES_RESPONSE_MODEL_SCALE",
            "message" => "Posterior predictive observed values and draws are reported on the transformed model scale; no inverse transform was applied.",
            "parameter" => nothing,
        ))
    end
    append!(warnings, bayes_diagnostic_warnings(summaries, draws * chains))

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

function bayes_write_posterior_predictive(path::String, chain, x::Vector{Float64}, y::Vector{Float64})
    values = bayes_chain_values(chain)
    available_names = String.(names(chain))
    a_index = findfirst(name -> name == "a", available_names)
    b_index = findfirst(name -> name == "b", available_names)
    sigma_index = findfirst(name -> name == "sigma", available_names)
    a_index === nothing && throw(ArgumentError("chain parameter `a` was not found"))
    b_index === nothing && throw(ArgumentError("chain parameter `b` was not found"))
    sigma_index === nothing && throw(ArgumentError("chain parameter `sigma` was not found"))

    observations = Int[]
    observed = Float64[]
    means = Float64[]
    q025 = Float64[]
    q975 = Float64[]

    for observation in eachindex(y)
        predictions = Float64[]
        for chain_index in axes(values, 3)
            for draw_index in axes(values, 1)
                a = Float64(values[draw_index, a_index, chain_index])
                b = Float64(values[draw_index, b_index, chain_index])
                sigma = abs(Float64(values[draw_index, sigma_index, chain_index]))
                mu = a * x[observation] + b
                push!(predictions, rand(Normal(mu, sigma)))
            end
        end
        sorted = sort(predictions)
        push!(observations, Int(observation))
        push!(observed, y[observation])
        push!(means, mean(sorted))
        push!(q025, quantile(sorted, 0.025))
        push!(q975, quantile(sorted, 0.975))
    end

    Arrow.write(path, (
        observation = observations,
        observed = observed,
        mean = means,
        q025 = q025,
        q975 = q975,
    ))
end

function bayes_write_samples(output_path::String, chain, chain_names::Vector{String}, model_names::Vector{String})
    values = bayes_chain_values(chain)
    available_names = String.(names(chain))
    parameters = String[]
    chains = Int[]
    draws = Int[]
    sample_values = Float64[]

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

    for (chain_name, model_name) in zip(chain_names, model_names)
        draw_index = findfirst(name -> name == chain_name, available_names)
        stats_index = findfirst(name -> name == chain_name, stats_names)
        draw_index === nothing && continue
        stats_index === nothing && continue

        draws = vec(values[:, draw_index, :])
        sorted = sort(collect(skipmissing(draws)))
        isempty(sorted) && continue
        push!(summaries, Dict(
            "parameter" => model_name,
            "mean" => finite_or_nothing(stats.mean[stats_index]),
            "sd" => finite_or_nothing(stats.std[stats_index]),
            "median" => quantile(sorted, 0.5),
            "q025" => quantile(sorted, 0.025),
            "q975" => quantile(sorted, 0.975),
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
