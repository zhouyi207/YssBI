using Arrow
using Distributions
using Logging
using MCMCChains
using Random
using Statistics
using Turing

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

function bayes_try_generic_normal_turing(model, table, input_rows::Int, task_id::String, output_path::String, metadata_path::String)
    input_rows >= 4 || throw(ArgumentError("generic Turing regression requires at least 4 observations"))
    likelihood = field(model, "likelihood")
    likelihood_type = String(field(likelihood, "type", ""))
    likelihood_type in ("normal", "bernoulli_logit", "poisson_log") || return nothing

    y_column = require_string(field(field(model, "response"), "column"), "response.column")
    y = bayes_response_vector(table, y_column, likelihood_type, task_id)
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

    model_instance = yssbi_generic_regression_model(table, data_variables, predictor, y, likelihood_type, parameter_names, priors, sigma_index, task_id)
    chain = with_logger(NullLogger()) do
        sample(model_instance, bayes_nuts_sampler(warmup, target_accept, max_tree_depth), MCMCSerial(), draws, chains; progress = false)
    end

    chain_names = ["theta[$index]" for index in eachindex(parameter_names)]
    summaries = bayes_chain_summaries(chain, chain_names, parameter_names)

    artifacts = Any[]
    if Bool(field(sampler, "saveSamples", false))
        bayes_write_samples(output_path, chain, chain_names, parameter_names)
        push!(artifacts, Dict(
            "kind" => "posterior_samples",
            "format" => "arrow_ipc",
            "path" => output_path,
            "rows" => nothing,
        ))
    end

    ppc_path = joinpath(dirname(output_path), "posterior_predictive.arrow")
    bayes_write_generic_posterior_predictive(ppc_path, chain, chain_names, parameter_names, table, data_variables, predictor, y, likelihood_type, sigma_parameter, task_id)
    push!(artifacts, Dict(
        "kind" => "posterior_predictive",
        "format" => "arrow_ipc",
        "path" => ppc_path,
        "rows" => nothing,
    ))

    warnings = Any[
        Dict(
            "code" => bayes_generic_warning_code(likelihood_type),
            "message" => "Generic $(bayes_likelihood_label(likelihood_type)) regression was sampled with Turing.jl from the safe predictor AST.",
            "parameter" => nothing,
        ),
    ]
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

function bayes_response_vector(table, column_name::String, likelihood_type::String, task_id::String)
    if likelihood_type == "normal"
        return bayes_numeric_vector(table, column_name, task_id)
    end

    column_symbol = Symbol(column_name)
    hasproperty(table, column_symbol) || throw(ArgumentError("column `$column_name` was not found"))
    column = getproperty(table, column_symbol)
    values = Vector{Int}(undef, length(column))
    for index in eachindex(column)
        check_cancelled(task_id)
        value = column[index]
        if likelihood_type == "bernoulli_logit"
            if value isa Bool
                values[index] = value ? 1 : 0
            elseif value isa Real && !(value isa Bool) && isfinite(Float64(value)) && (Float64(value) == 0.0 || Float64(value) == 1.0)
                values[index] = Int(Float64(value))
            else
                throw(ArgumentError("BernoulliLogit response column `$column_name` must contain boolean or 0/1 values"))
            end
        elseif likelihood_type == "poisson_log"
            value isa Real && !(value isa Bool) || throw(ArgumentError("PoissonLog response column `$column_name` must contain non-negative integer counts"))
            numeric = Float64(value)
            isfinite(numeric) && numeric >= 0.0 && numeric == floor(numeric) || throw(ArgumentError("PoissonLog response column `$column_name` must contain non-negative integer counts"))
            values[index] = Int(numeric)
        else
            throw(ArgumentError("unsupported response likelihood `$likelihood_type`"))
        end
    end
    return values
end

function bayes_write_generic_posterior_predictive(path::String, chain, chain_names::Vector{String}, model_names::Vector{String}, table, data_variables, predictor, y, likelihood_type::String, sigma_parameter, task_id::String)
    values = Array(chain)
    available_names = String.(names(chain))
    parameter_indices = Dict{String, Int}()
    for (chain_name, model_name) in zip(chain_names, model_names)
        parameter_index = findfirst(name -> name == chain_name, available_names)
        parameter_index === nothing && throw(ArgumentError("chain parameter `$chain_name` was not found"))
        parameter_indices[model_name] = parameter_index
    end
    likelihood_type == "normal" && !haskey(parameter_indices, sigma_parameter) && throw(ArgumentError("chain sigma parameter `$sigma_parameter` was not found"))

    observations = Int[]
    observed = Float64[]
    means = Float64[]
    q025 = Float64[]
    q975 = Float64[]

    for observation in eachindex(y)
        predictions = Float64[]
        for chain_index in axes(values, 3)
            for draw_index in axes(values, 1)
                check_cancelled(task_id)
                params = Dict{String, Any}()
                for model_name in model_names
                    params[model_name] = values[draw_index, parameter_indices[model_name], chain_index]
                end
                linear_predictor = bayes_evaluate_expression(predictor, table, data_variables, params, observation, task_id)
                prediction = if likelihood_type == "normal"
                    sigma = abs(Float64(params[sigma_parameter]))
                    rand(Normal(Float64(linear_predictor), sigma))
                elseif likelihood_type == "bernoulli_logit"
                    Float64(rand(Bernoulli(bayes_logistic(linear_predictor))))
                elseif likelihood_type == "poisson_log"
                    Float64(rand(Poisson(exp(Float64(linear_predictor)))))
                else
                    throw(ArgumentError("unsupported posterior predictive likelihood `$likelihood_type`"))
                end
                push!(predictions, prediction)
            end
        end
        sorted = sort(predictions)
        push!(observations, Int(observation))
        push!(observed, Float64(y[observation]))
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

function bayes_logistic(value)
    if value >= zero(value)
        z = exp(-value)
        return one(value) / (one(value) + z)
    end
    z = exp(value)
    return z / (one(value) + z)
end

function bayes_generic_warning_code(likelihood_type::String)
    likelihood_type == "normal" && return "JULIA_BAYES_TURING_GENERIC_NORMAL"
    likelihood_type == "bernoulli_logit" && return "JULIA_BAYES_TURING_GENERIC_BERNOULLI_LOGIT"
    likelihood_type == "poisson_log" && return "JULIA_BAYES_TURING_GENERIC_POISSON_LOG"
    return "JULIA_BAYES_TURING_GENERIC_MODEL"
end

function bayes_likelihood_label(likelihood_type::String)
    likelihood_type == "normal" && return "Normal"
    likelihood_type == "bernoulli_logit" && return "BernoulliLogit"
    likelihood_type == "poisson_log" && return "PoissonLog"
    return likelihood_type
end
