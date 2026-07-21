#!/usr/bin/env julia

using Arrow
using JSON3

const CANCELLED_TASK_IDS = Set{String}()
const CANCEL_LOCK = ReentrantLock()
const STDOUT_LOCK = ReentrantLock()
const ACTIVE_TASKS = Task[]
const ACTIVE_TASKS_LOCK = ReentrantLock()

struct TaskCancelled <: Exception
    task_id::String
end

function field(value, name::String, default = nothing)
    value === nothing && return default
    property = Symbol(name)
    return hasproperty(value, property) ? getproperty(value, property) : default
end

function send_message(message)
    encoded = JSON3.write(message)
    lock(STDOUT_LOCK) do
        println(stdout, encoded)
        flush(stdout)
    end
end



function send_progress(task_id::String, stage::String; completed = nothing, total = nothing)
    send_message(Dict(
        "jsonrpc" => "2.0",
        "method" => "progress",
        "params" => Dict(
            "taskId" => task_id,
            "stage" => stage,
            "completed" => completed,
            "total" => total,
        ),
    ))
end

function send_error(request_id, code::String, message::String; data = nothing)
    error = Dict{String, Any}("code" => code, "message" => message)
    data !== nothing && (error["data"] = data)
    send_message(Dict("jsonrpc" => "2.0", "id" => request_id, "error" => error))
end

function is_cancelled(task_id::String)
    lock(CANCEL_LOCK) do
        return task_id in CANCELLED_TASK_IDS
    end
end

function check_cancelled(task_id::String)
    is_cancelled(task_id) && throw(TaskCancelled(task_id))
    yield()
    is_cancelled(task_id) && throw(TaskCancelled(task_id))
end

function require_string(value, name::String)
    value isa AbstractString && !isempty(value) && return String(value)
    throw(ArgumentError("`$name` must be a non-empty string"))
end

include(joinpath(@__DIR__, "ops", "acf_pacf.jl"))
include(joinpath(@__DIR__, "ops", "serial_tests.jl"))
include(joinpath(@__DIR__, "ops", "bayes_fit.jl"))

const OPERATIONS = Dict{String, Function}(
    "acf_pacf" => run_acf_pacf,
    "serial_tests" => run_serial_tests,
    "bayes_fit" => run_bayes_fit,
)

function run_operation(operation::String, params, task_id::String)
    handler = get(OPERATIONS, operation, nothing)
    handler === nothing && throw(ArgumentError("unsupported operation `$operation`"))
    return handler(params, task_id)
end

function process_run(request, request_id, params)
    task_id_value = field(params, "taskId")
    task_id = require_string(task_id_value, "taskId")
    operation = field(params, "operation", field(request, "operation"))

    try
        send_progress(task_id, "loading_model")
        result = run_operation(String(operation), params, task_id)
        request_id !== nothing && send_message(Dict("jsonrpc" => "2.0", "id" => request_id, "result" => result))
    catch error
        if error isa TaskCancelled
            request_id !== nothing && send_error(request_id, "cancelled", "Task was cancelled";
                data = Dict("taskId" => task_id))
        elseif error isa ArgumentError
            request_id !== nothing && send_error(request_id, "invalid_parameters", error.msg;
                data = Dict("taskId" => task_id))
        else
            detail = sprint(showerror, error)
            println(stderr, "Julia worker task $task_id failed: ", detail)
            request_id !== nothing && send_error(request_id, "internal_error", "Task failed: $detail";
                data = Dict("taskId" => task_id))
        end
    finally
        lock(CANCEL_LOCK) do
            delete!(CANCELLED_TASK_IDS, task_id)
        end
    end
end

function handle_message(request)
    field(request, "jsonrpc") == "2.0" || throw(ArgumentError("`jsonrpc` must be `2.0`"))
    method = field(request, "method")
    method isa AbstractString || throw(ArgumentError("`method` must be a string"))
    params = field(request, "params", nothing)

    if method == "cancel"
        task_id = require_string(field(params, "taskId"), "taskId")
        lock(CANCEL_LOCK) do
            push!(CANCELLED_TASK_IDS, task_id)
        end
        return
    end

    request_id = field(request, "id", nothing)
    method == "run" || throw(ArgumentError("unsupported method `$method`"))
    params === nothing && throw(ArgumentError("`params` is required"))
    worker = @async process_run(request, request_id, params)
    lock(ACTIVE_TASKS_LOCK) do
        push!(ACTIVE_TASKS, worker)
    end
end

function control_reader()
    for line in eachline(stdin)
        isempty(strip(line)) && continue
        request = try
            JSON3.read(line)
        catch error
            println(stderr, "Julia worker ignored malformed JSON: ", sprint(showerror, error))
            continue
        end

        try
            handle_message(request)
        catch error
            request_id = field(request, "id", nothing)
            request_id !== nothing && send_error(request_id, "invalid_request", sprint(showerror, error))
        end
    end
end

reader = @async control_reader()
wait(reader)
lock(ACTIVE_TASKS_LOCK) do
    for worker in ACTIVE_TASKS
        istaskdone(worker) || wait(worker)
    end
end
