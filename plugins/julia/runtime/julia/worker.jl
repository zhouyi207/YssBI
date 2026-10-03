#!/usr/bin/env julia

include(joinpath(@__DIR__, "worker_protocol.jl"))
include(joinpath(@__DIR__, "scientific_runtime.jl"))

mutable struct WorkerTaskRegistration
    task_id::String
    task::Task
    cancelled::Bool
end

const STDOUT_LOCK = ReentrantLock()
const ACTIVE_TASKS = WorkerTaskRegistration[]
const ACTIVE_TASKS_LOCK = ReentrantLock()


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
    lock(ACTIVE_TASKS_LOCK) do
        return any(entry -> entry.task_id == task_id && entry.cancelled, ACTIVE_TASKS)
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

include(joinpath(@__DIR__, "ops", "bayes", "expression.jl"))
include(joinpath(@__DIR__, "ops", "bayes_fit.jl"))
include(joinpath(@__DIR__, "ops", "bayes", "runtime.jl"))
include(joinpath(@__DIR__, "ops", "bayes", "turing_generic_normal.jl"))

const OPERATIONS = Dict{String, Function}(
    "bayes_fit" => run_bayes_fit,
)

function run_operation(operation::String, params, task_id::String)
    handler = get(OPERATIONS, operation, nothing)
    handler === nothing && throw(ArgumentError("unsupported operation `$operation`"))
    return handler(params, task_id)
end

function process_run(request, request_id, params, task_id::String)
    operation = field(params, "operation", field(request, "operation"))

    try
        send_progress(task_id, "loading_model")
        result = run_operation(String(operation), params, task_id)
        request_id !== nothing && send_message(Dict("jsonrpc" => "2.0", "id" => request_id, "result" => result))
    catch error
        task_error = normalize_worker_error(error, task_id)
        if task_error.code == WorkerInternal
            println(stderr, "Julia worker task $task_id failed: ", task_error.diagnostic)
        end
        request_id !== nothing && send_error(
            request_id,
            worker_error_code(task_error),
            task_error.diagnostic;
            data = task_error.data,
        )
    end
end

function handle_message(request)
    field(request, "jsonrpc") == "2.0" || throw(ArgumentError("`jsonrpc` must be `2.0`"))
    method = field(request, "method")
    method isa AbstractString || throw(ArgumentError("`method` must be a string"))
    params = field(request, "params", nothing)

    if method == "ping"
        request_id = field(request, "id", nothing)
        request_id !== nothing && send_message(Dict(
            "jsonrpc" => "2.0",
            "id" => request_id,
            "result" => Dict("ready" => true),
        ))
        return
    end

    if method == "cancel"
        task_id = require_string(field(params, "taskId"), "taskId")
        lock(ACTIVE_TASKS_LOCK) do
            for entry in ACTIVE_TASKS
                entry.task_id == task_id && (entry.cancelled = true)
            end
        end
        return
    end

    request_id = field(request, "id", nothing)
    method == "run" || throw(ArgumentError("unsupported method `$method`"))
    params === nothing && throw(ArgumentError("`params` is required"))
    task_id = require_string(field(params, "taskId"), "taskId")
    worker = Task() do
        try
            process_run(request, request_id, params, task_id)
        finally
            lock(ACTIVE_TASKS_LOCK) do
                filter!(entry -> entry.task !== current_task(), ACTIVE_TASKS)
            end
        end
    end
    lock(ACTIVE_TASKS_LOCK) do
        push!(ACTIVE_TASKS, WorkerTaskRegistration(task_id, worker, false))
    end
    schedule(worker)
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
workers = lock(ACTIVE_TASKS_LOCK) do
    [entry.task for entry in ACTIVE_TASKS]
end
for worker in workers
    istaskdone(worker) || wait(worker)
end
