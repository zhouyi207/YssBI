using Test

include(joinpath(@__DIR__, "..", "worker_protocol.jl"))

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
