



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



function bayes_normal_sigma_parameter(model)
    likelihood = field(model, "likelihood")
    field(likelihood, "type", "") == "normal" || return nothing
    sigma = field(likelihood, "sigma", nothing)
    sigma === nothing && return nothing
    return require_string(field(sigma, "parameter"), "likelihood.sigma.parameter")
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
