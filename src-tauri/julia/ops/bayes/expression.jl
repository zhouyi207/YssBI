function bayes_prior_default(prior)
    distribution = String(field(prior, "distribution", "normal"))
    args = field(prior, "args", Any[])

    if distribution in ("normal", "log_normal", "student_t", "cauchy")
        return Float64(args[1])
    elseif distribution == "uniform"
        return (Float64(args[1]) + Float64(args[2])) / 2.0
    elseif distribution == "beta"
        alpha = Float64(args[1])
        beta = Float64(args[2])
        return alpha / (alpha + beta)
    elseif distribution == "gamma"
        shape = Float64(args[1])
        rate = Float64(args[2])
        return shape / rate
    elseif distribution == "exponential"
        rate = Float64(args[1])
        return 1.0 / rate
    elseif distribution == "half_normal"
        sigma = Float64(args[1])
        return sigma * sqrt(2.0 / pi)
    end

    throw(ArgumentError("unsupported prior distribution `$distribution`"))
end

function bayes_parameter_defaults(parameter_specs)
    defaults = Dict{String, Float64}()
    for parameter in parameter_specs
        name = require_string(field(parameter, "name"), "parameter.name")
        prior = field(parameter, "prior")
        prior === nothing && throw(ArgumentError("parameter `$name` is missing prior"))
        defaults[name] = bayes_prior_default(prior)
    end
    return defaults
end

function bayes_column_value(table, column_name::String, row_index::Int)
    column_symbol = Symbol(column_name)
    hasproperty(table, column_symbol) || throw(ArgumentError("column `$column_name` was not found"))
    value = getproperty(table, column_symbol)[row_index]
    value isa Real && !(value isa Bool) || throw(ArgumentError("column `$column_name` must contain numeric values"))
    numeric_value = Float64(value)
    isfinite(numeric_value) || throw(ArgumentError("column `$column_name` must contain finite values"))
    return numeric_value
end

function bayes_eval_function(name::String, args)
    if name == "exp"
        length(args) == 1 || throw(ArgumentError("exp expects one argument"))
        return exp(args[1])
    elseif name == "ln"
        length(args) == 1 || throw(ArgumentError("ln expects one argument"))
        args[1] > 0 || throw(DomainError(args[1], "ln argument must be greater than zero"))
        return log(args[1])
    elseif name == "sqrt"
        length(args) == 1 || throw(ArgumentError("sqrt expects one argument"))
        return sqrt(args[1])
    elseif name == "abs"
        length(args) == 1 || throw(ArgumentError("abs expects one argument"))
        return abs(args[1])
    elseif name == "sin"
        length(args) == 1 || throw(ArgumentError("sin expects one argument"))
        return sin(args[1])
    elseif name == "cos"
        length(args) == 1 || throw(ArgumentError("cos expects one argument"))
        return cos(args[1])
    elseif name == "min"
        length(args) >= 2 || throw(ArgumentError("min expects at least two arguments"))
        return minimum(args)
    elseif name == "max"
        length(args) >= 2 || throw(ArgumentError("max expects at least two arguments"))
        return maximum(args)
    end

    throw(ArgumentError("unsupported expression function `$name`"))
end

function bayes_evaluate_expression(expr, table, data_variables, parameters, row_index::Int, task_id::String)
    check_cancelled(task_id)
    node_type = require_string(field(expr, "type"), "expression.type")

    if node_type == "number"
        return Float64(field(expr, "value"))
    elseif node_type == "data_variable"
        name = require_string(field(expr, "name"), "expression.name")
        column_name = require_string(field(data_variables, name), "dataVariables.$name")
        return bayes_column_value(table, column_name, row_index)
    elseif node_type == "column"
        name = require_string(field(expr, "name"), "expression.name")
        return bayes_column_value(table, name, row_index)
    elseif node_type == "parameter"
        name = require_string(field(expr, "name"), "expression.name")
        haskey(parameters, name) || throw(ArgumentError("parameter `$name` was not found"))
        return parameters[name]
    elseif node_type == "unary"
        op = require_string(field(expr, "op"), "expression.op")
        arg = bayes_evaluate_expression(field(expr, "arg"), table, data_variables, parameters, row_index, task_id)
        op == "neg" && return -arg
        throw(ArgumentError("unsupported unary operator `$op`"))
    elseif node_type == "binary"
        op = require_string(field(expr, "op"), "expression.op")
        left = bayes_evaluate_expression(field(expr, "left"), table, data_variables, parameters, row_index, task_id)
        right = bayes_evaluate_expression(field(expr, "right"), table, data_variables, parameters, row_index, task_id)
        op == "add" && return left + right
        op == "sub" && return left - right
        op == "mul" && return left * right
        op == "div" && return left / right
        op == "pow" && return left ^ right
        throw(ArgumentError("unsupported binary operator `$op`"))
    elseif node_type == "call"
        function_name = require_string(field(expr, "function"), "expression.function")
        raw_args = field(expr, "args", Any[])
        values = Any[
            bayes_evaluate_expression(arg, table, data_variables, parameters, row_index, task_id)
            for arg in raw_args
        ]
        return bayes_eval_function(function_name, values)
    end

    throw(ArgumentError("unsupported expression node `$node_type`"))
end

function bayes_response_vector(table, response, likelihood_type::String, task_id::String)
    expression = field(response, "expression", nothing)
    expression === nothing && throw(ArgumentError("response.expression is required"))
    data_variables = field(response, "dataVariables", nothing)
    data_variables === nothing && throw(ArgumentError("response.dataVariables is required"))

    if likelihood_type != "normal"
        field(expression, "type", "") == "data_variable" || throw(ArgumentError("$likelihood_type requires an identity response expression"))
    end

    row_count = isempty(propertynames(table)) ? 0 : length(getproperty(table, first(propertynames(table))))
    if likelihood_type == "normal"
        values = Vector{Float64}(undef, row_count)
        parameters = Dict{String, Float64}()
        for row_index in 1:row_count
            value = bayes_evaluate_expression(expression, table, data_variables, parameters, row_index, task_id)
            isfinite(value) || throw(ArgumentError("response expression returned a non-finite value at row $row_index"))
            values[row_index] = Float64(value)
        end
        return values
    end

    symbol = require_string(field(expression, "name"), "response.expression.name")
    column_name = require_string(field(data_variables, symbol), "response.dataVariables.$symbol")
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
            elseif value isa Real && !(value isa Bool) && isfinite(Float64(value)) && Float64(value) in (0.0, 1.0)
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

function bayes_response_is_transformed(response)
    return field(field(response, "expression", nothing), "type", "") != "data_variable"
end

function bayes_predictor_preview(model, table, input_rows::Int, task_id::String)
    input_rows <= 0 && return Float64[]
    predictor = field(model, "predictor")
    predictor === nothing && throw(ArgumentError("model predictor is required"))
    data_variables = field(model, "dataVariables", nothing)
    data_variables === nothing && throw(ArgumentError("model dataVariables are required"))
    parameter_specs = field(model, "parameters", Any[])
    parameters = bayes_parameter_defaults(parameter_specs)

    preview_count = min(input_rows, 5)
    values = Float64[]
    for row_index in 1:preview_count
        value = bayes_evaluate_expression(predictor, table, data_variables, parameters, row_index, task_id)
        isfinite(value) || throw(ArgumentError("predictor returned a non-finite value at row $row_index"))
        push!(values, value)
    end
    return values
end
