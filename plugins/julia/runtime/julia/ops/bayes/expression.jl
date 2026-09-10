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



function bayes_column_value(table, column_name::String, row_index::Int)
    column_symbol = Symbol(column_name)
    hasproperty(table, column_symbol) || throw(invalid_parameters_error(
        "column `$column_name` was not found"; column = column_name,
    ))
    value = getproperty(table, column_symbol)[row_index]
    value isa Real && !(value isa Bool) || throw(invalid_parameters_error(
        "column `$column_name` must contain numeric values";
        column = column_name,
        row = row_index,
    ))
    numeric_value = Float64(value)
    isfinite(numeric_value) || throw(invalid_parameters_error(
        "column `$column_name` must contain finite values";
        column = column_name,
        row = row_index,
    ))
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

function bayes_evaluate_response_expression(expr, table, data_variables, row_index::Int, task_id::String)
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

    elseif node_type == "unary"
        op = require_string(field(expr, "op"), "expression.op")
        arg = bayes_evaluate_response_expression(field(expr, "arg"), table, data_variables, row_index, task_id)
        op == "neg" && return -arg
        throw(ArgumentError("unsupported unary operator `$op`"))
    elseif node_type == "binary"
        op = require_string(field(expr, "op"), "expression.op")
        left = bayes_evaluate_response_expression(field(expr, "left"), table, data_variables, row_index, task_id)
        right = bayes_evaluate_response_expression(field(expr, "right"), table, data_variables, row_index, task_id)
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
            bayes_evaluate_response_expression(arg, table, data_variables, row_index, task_id)
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
        for row_index in 1:row_count
            row_index % 256 == 1 && check_cancelled(task_id)
            value = bayes_evaluate_response_expression(expression, table, data_variables, row_index, task_id)
            isfinite(value) || throw(invalid_parameters_error(
                "response expression returned a non-finite value at row $row_index";
                row = row_index,
                path = "model.response.expression",
            ))
            values[row_index] = Float64(value)
        end
        return values
    end

    symbol = require_string(field(expression, "name"), "response.expression.name")
    column_name = require_string(field(data_variables, symbol), "response.dataVariables.$symbol")
    column_symbol = Symbol(column_name)
    hasproperty(table, column_symbol) || throw(invalid_parameters_error(
        "column `$column_name` was not found"; column = column_name,
    ))
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
                throw(invalid_parameters_error(
                    "BernoulliLogit response column `$column_name` must contain boolean or 0/1 values";
                    column = column_name,
                    row = index,
                ))
            end
        elseif likelihood_type == "poisson_log"
            value isa Real && !(value isa Bool) || throw(invalid_parameters_error(
                "PoissonLog response column `$column_name` must contain non-negative integer counts";
                column = column_name,
                row = index,
            ))
            numeric = Float64(value)
            isfinite(numeric) && numeric >= 0.0 && numeric == floor(numeric) || throw(invalid_parameters_error(
                "PoissonLog response column `$column_name` must contain non-negative integer counts";
                column = column_name,
                row = index,
            ))
            values[index] = Int(numeric)
        else
            throw(ArgumentError("unsupported response likelihood `$likelihood_type`"))
        end
    end
    return values
end

function bayes_response_transform(response)::String
    expression = field(response, "expression", nothing)
    field(expression, "type", "") == "data_variable" && return "identity"
    field(expression, "type", "") == "call" && field(expression, "function", "") == "ln" && return "ln"
    throw(ArgumentError("response expression does not have a supported inverse transform"))
end

function bayes_response_is_transformed(response)
    return bayes_response_transform(response) != "identity"
end

function bayes_inverse_response(response, value::Real)::Float64
    transform = bayes_response_transform(response)
    transform == "identity" && return Float64(value)
    transform == "ln" && return exp(Float64(value))
    throw(ArgumentError("unsupported response inverse transform `$transform`"))
end

function bayes_predictive_scale_summaries(response, predictions::Vector{Float64})
    transform = bayes_response_transform(response)
    model_mean = mean(predictions)
    sort!(predictions)
    model_q025, model_q975 = quantile(predictions, [0.025, 0.975]; sorted = true)
    if transform == "identity"
        return (
            model_mean = model_mean,
            model_q025 = model_q025,
            model_q975 = model_q975,
            original_mean = model_mean,
            original_q025 = model_q025,
            original_q975 = model_q975,
        )
    end

    original_sum = 0.0
    for prediction in predictions
        value = bayes_inverse_response(response, prediction)
        isfinite(value) || throw(ArgumentError("posterior predictive inverse transform produced a non-finite value"))
        original_sum += value
    end
    return (
        model_mean = model_mean,
        model_q025 = model_q025,
        model_q975 = model_q975,
        original_mean = original_sum / length(predictions),
        original_q025 = bayes_inverse_response(response, model_q025),
        original_q975 = bayes_inverse_response(response, model_q975),
    )
end
