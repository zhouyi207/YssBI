using Test

mktemp() do _, input
    redirect_stdin(input) do
        include(joinpath(@__DIR__, "..", "worker.jl"))
    end
end

@testset "Worker capability errors retain stable wire details" begin
    normalized = normalize_worker_error(
        UnsupportedBayesCapability(
            "prior is unsupported";
            parameter = "beta",
            path = "model.parameters[1].prior",
        ),
        "task-7",
    )

    @test worker_error_code(normalized) == "unsupported_capability"
    @test normalized.data == Dict(
        "taskId" => "task-7",
        "parameter" => "beta",
        "path" => "model.parameters[1].prior",
    )
end

@testset "Sampling failures preserve cancellation semantics" begin
    failure = try
        with_sampling_error(() -> error("sampler exploded"))
    catch error
        error
    end

    @test failure isa WorkerTaskError
    @test worker_error_code(failure) == "sampling_failed"
    @test_throws TaskCancelled with_sampling_error(() -> throw(TaskCancelled("task-8")))
end

@testset "Worker completion releases tasks and retains active cancellation" begin
    started = Channel{Nothing}(1)
    resume = Channel{Nothing}(1)
    OPERATIONS["test_lifecycle"] = function (_, task_id)
        if task_id == "pending"
            put!(started, nothing)
            take!(resume)
        end
        check_cancelled(task_id)
        return Dict("completed" => true)
    end
    request(task_id) = JSON3.read("""
        {"jsonrpc":"2.0","id":"$task_id","method":"run",
         "params":{"taskId":"$task_id","operation":"test_lifecycle"}}
    """)
    try
        mktemp() do _, output
            snapshots = Vector{Task}[]
            pending_task = Ref{Task}()
            late_cancelled = Ref(false)
            redirect_stdout(output) do
                handle_message(request("completed"))
                completed = last(ACTIVE_TASKS).task
                wait(completed)
                push!(snapshots, [entry.task for entry in ACTIVE_TASKS])
                handle_message(JSON3.read("""
                    {"jsonrpc":"2.0","method":"cancel","params":{"taskId":"completed"}}
                """))
                late_cancelled[] = is_cancelled("completed")

                handle_message(request("admitted"))
                admitted = last(ACTIVE_TASKS).task
                handle_message(JSON3.read("""
                    {"jsonrpc":"2.0","method":"cancel","params":{"taskId":"admitted"}}
                """))
                wait(admitted)

                handle_message(request("pending"))
                pending = last(ACTIVE_TASKS).task
                pending_task[] = pending
                take!(started)
                push!(snapshots, [entry.task for entry in ACTIVE_TASKS])

                handle_message(request("successor"))
                successor = last(ACTIVE_TASKS).task
                wait(successor)
                push!(snapshots, [entry.task for entry in ACTIVE_TASKS])

                handle_message(JSON3.read("""
                    {"jsonrpc":"2.0","method":"cancel","params":{"taskId":"pending"}}
                """))
                put!(resume, nothing)
                wait(pending)
                push!(snapshots, [entry.task for entry in ACTIVE_TASKS])
            end
            @test isempty(snapshots[1])
            @test snapshots[2] == [pending_task[]]
            @test snapshots[3] == [pending_task[]]
            @test isempty(snapshots[4])
            @test !late_cancelled[]
            @test !is_cancelled("completed")
            @test !is_cancelled("admitted")
            @test !is_cancelled("pending")
            seekstart(output)
            replies = JSON3.read.(readlines(output))
            cancelled = only(filter(reply -> field(reply, "id") == "pending", replies))
            @test cancelled.error.code == "cancelled"
            @test cancelled.error.data.taskId == "pending"
            admitted = only(filter(reply -> field(reply, "id") == "admitted", replies))
            @test admitted.error.code == "cancelled"
        end
    finally
        delete!(OPERATIONS, "test_lifecycle")
    end

    # A task finishing after EOF must be able to release its registration.
    mktemp() do _, input
        println(input, """{"jsonrpc":"2.0","id":"eof","method":"run","params":{"taskId":"eof","operation":"unsupported"}}""")
        println(input, """{"jsonrpc":"2.0","id":"invalid-id","method":"run","params":{"taskId":"","operation":"unsupported"}}""")
        seekstart(input)
        mktemp() do _, output
            project = normpath(joinpath(@__DIR__, ".."))
            worker = joinpath(project, "worker.jl")
            command = `$(Base.julia_cmd()) --startup-file=no --project=$project $worker`
            process = run(pipeline(command; stdin = input, stdout = output); wait = false)
            exited = timedwait(() -> process_exited(process), 120.0)
            if exited != :ok
                kill(process)
            end
            wait(process)
            @test exited == :ok
            @test success(process)
            seekstart(output)
            replies = JSON3.read.(readlines(output))
            reply = only(filter(message -> field(message, "id") == "eof", replies))
            @test reply.error.code == "invalid_parameters"
            invalid = only(filter(message -> field(message, "id") == "invalid-id", replies))
            @test invalid.error.code == "invalid_request"
        end
    end
end
