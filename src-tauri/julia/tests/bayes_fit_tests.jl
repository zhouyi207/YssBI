using Test

include(joinpath(@__DIR__, "..", "ops", "bayes_fit.jl"))
include(joinpath(@__DIR__, "..", "ops", "bayes", "turing_linear.jl"))

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
