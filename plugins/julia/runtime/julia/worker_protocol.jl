@enum WorkerTaskErrorCode begin
    WorkerInvalidParameters
    WorkerUnsupportedCapability
    WorkerPackageUnavailable
    WorkerSamplingFailed
    WorkerCancelled
    WorkerInternal
end

struct TaskCancelled <: Exception
    task_id::String
end

struct WorkerTaskError <: Exception
    code::WorkerTaskErrorCode
    diagnostic::String
    data::Dict{String, Any}
end

struct UnsupportedBayesCapability <: Exception
    message::String
    data::Dict{String, Any}
end

function worker_error_data(;
    task_id = nothing,
    column = nothing,
    row = nothing,
    parameter = nothing,
    path = nothing,
)
    data = Dict{String, Any}()
    task_id !== nothing && (data["taskId"] = String(task_id))
    column !== nothing && (data["column"] = String(column))
    row !== nothing && (data["row"] = Int(row))
    parameter !== nothing && (data["parameter"] = String(parameter))
    path !== nothing && (data["path"] = String(path))
    return data
end

function WorkerTaskError(
    code::WorkerTaskErrorCode,
    diagnostic::AbstractString;
    column = nothing,
    row = nothing,
    parameter = nothing,
    path = nothing,
)
    return WorkerTaskError(
        code,
        String(diagnostic),
        worker_error_data(;
            column = column,
            row = row,
            parameter = parameter,
            path = path,
        ),
    )
end

function UnsupportedBayesCapability(
    message::AbstractString;
    parameter = nothing,
    path = nothing,
)
    return UnsupportedBayesCapability(
        String(message),
        worker_error_data(; parameter = parameter, path = path),
    )
end

Base.showerror(io::IO, error::TaskCancelled) = print(io, "task cancelled: ", error.task_id)
Base.showerror(io::IO, error::WorkerTaskError) = print(io, error.diagnostic)
Base.showerror(io::IO, error::UnsupportedBayesCapability) =
    print(io, "unsupported capability: ", error.message)

function invalid_parameters_error(
    diagnostic::AbstractString;
    column = nothing,
    row = nothing,
    parameter = nothing,
    path = nothing,
)
    return WorkerTaskError(
        WorkerInvalidParameters,
        diagnostic;
        column = column,
        row = row,
        parameter = parameter,
        path = path,
    )
end

function with_sampling_error(action::Function)
    try
        return action()
    catch error
        error isa TaskCancelled && rethrow()
        throw(WorkerTaskError(WorkerSamplingFailed, sprint(showerror, error)))
    end
end

function normalize_worker_error(error, task_id::String)
    normalized = if error isa WorkerTaskError
        error
    elseif error isa TaskCancelled
        WorkerTaskError(WorkerCancelled, "Task was cancelled")
    elseif error isa UnsupportedBayesCapability
        WorkerTaskError(WorkerUnsupportedCapability, sprint(showerror, error), copy(error.data))
    elseif error isa ArgumentError
        WorkerTaskError(WorkerInvalidParameters, sprint(showerror, error))
    else
        WorkerTaskError(WorkerInternal, sprint(showerror, error))
    end

    data = copy(normalized.data)
    data["taskId"] = task_id
    return WorkerTaskError(normalized.code, normalized.diagnostic, data)
end

function worker_error_code(error::WorkerTaskError)::String
    error.code == WorkerInvalidParameters && return "invalid_parameters"
    error.code == WorkerUnsupportedCapability && return "unsupported_capability"
    error.code == WorkerPackageUnavailable && return "package_unavailable"
    error.code == WorkerSamplingFailed && return "sampling_failed"
    error.code == WorkerCancelled && return "cancelled"
    return "internal_error"
end
