using JSON3
using Test

include(joinpath(@__DIR__, "..", "ops", "bayes_fit.jl"))
include(joinpath(@__DIR__, "..", "ops", "bayes", "turing_linear.jl"))

if !isdefined(Main, :check_cancelled)
    check_cancelled(::String) = nothing
end
if !isdefined(Main, :field)
    field(value, name::String, default = nothing) = haskey(value, name) ? value[name] : default
end
if !isdefined(Main, :require_string)
    require_string(value, name::String) = value isa AbstractString ? String(value) : throw(ArgumentError("`$name` must be a string"))
end

@testset "Transformed response uses safe ln evaluator" begin
    response = JSON3.read("""
        {
          "expression": {
            "type": "call",
            "function": "ln",
            "args": [{"type": "data_variable", "name": "y"}]
          },
          "dataVariables": {"y": "response"}
        }
    """)
    table = (response = [1.0, exp(1.0), exp(2.0)],)
    @test bayes_response_vector(table, response, "normal", "task") ≈ [0.0, 1.0, 2.0]

    legacy = JSON3.read("""
        {
          "type": "call",
          "function": "log",
          "args": [{"type": "number", "value": 1.0}]
        }
    """)
    @test_throws ArgumentError bayes_evaluate_expression(
        legacy,
        NamedTuple(),
        JSON3.read("{}"),
        Dict{String, Float64}(),
        1,
        "task",
    )
end

@testset "Bayesian capability and failure boundaries" begin
    unsupported = UnsupportedBayesCapability("test model")
    @test sprint(showerror, unsupported) == "unsupported capability: test model"

    @eval function bayes_run_fixed_linear_turing(model, table, input_rows::Int, task_id::String, output_path::String, metadata_path::String)
        error("sampling failed")
    end
    @test_throws ErrorException bayes_try_turing_linear_fit(nothing, nothing, 4, "task", "output", "metadata")
end

@testset "Artifact manifest is the only result path source" begin
    result = Dict{String, Any}(
        "summaries" => Any[],
        "artifacts" => Any[
            Dict(
                "kind" => "posterior_samples",
                "format" => "arrow_ipc",
                "path" => "samples.arrow",
                "rows" => nothing,
            ),
        ],
    )

    bayes_attach_artifact_manifest!(result, "task", "summary.json", "metadata.json")

    @test Set(keys(result)) == Set(["summaries", "artifactManifest"])
    @test Set(keys(result["artifactManifest"])) == Set(["taskId", "artifacts"])
    @test [artifact["kind"] for artifact in result["artifactManifest"]["artifacts"]] ==
          ["summary", "metadata", "posterior_samples"]
end

@testset "MCMC chain dimensions remain distinct" begin
    chain = Chains(
        reshape(collect(1.0:80.0), 20, 1, 4),
        [:theta],
    )

    values = bayes_chain_values(chain)
    @test size(values) == (20, 1, 4)

    mktemp() do path, io
        close(io)
        bayes_write_samples(path, chain, ["theta"], ["theta"])
        samples = Arrow.Table(read(path))
        @test unique(samples.chain) == [1, 2, 3, 4]
        @test count(==(1), samples.chain) == 20
        @test count(==(4), samples.chain) == 20
    end
end

@testset "NUTS tree depth configuration and diagnostics" begin
    sampler = bayes_nuts_sampler(10, 0.8, 7)
    @test getfield(sampler, :max_depth) == 7

    chain = Chains(
        reshape([6.0, 7.0, 8.0], 3, 1, 1),
        [:tree_depth],
        Dict(:internals => [:tree_depth]),
    )
    @test bayes_max_treedepth_hits(chain, 7) == 2

    parameter_chain = Chains(reshape([1.0, 2.0], 2, 1, 1), [:theta])
    @test bayes_max_treedepth_hits(parameter_chain, 7) === nothing
end
