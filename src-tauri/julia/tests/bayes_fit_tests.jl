using Test

include(joinpath(@__DIR__, "..", "worker_protocol.jl"))
include(joinpath(@__DIR__, "..", "scientific_runtime.jl"))
include(joinpath(@__DIR__, "..", "ops", "bayes", "expression.jl"))
include(joinpath(@__DIR__, "..", "ops", "bayes_fit.jl"))
include(joinpath(@__DIR__, "..", "ops", "bayes", "runtime.jl"))
include(joinpath(@__DIR__, "..", "ops", "bayes", "turing_generic_normal.jl"))

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
    @test bayes_response_transform(response) == "ln"
    @test bayes_inverse_response(response, log(3.0)) ≈ 3.0

    unsupported = JSON3.read("""
        {
          "type": "call",
          "function": "tan",
          "args": [{"type": "number", "value": 1.0}]
        }
    """)
    @test_throws ArgumentError bayes_evaluate_response_expression(
        unsupported,
        NamedTuple(),
        JSON3.read("{}"),
        1,
        "task",
    )
end

@testset "Generated predictors load with contiguous columns" begin
    table = (time = [0.0, 1.0, 2.0],)
    mktemp() do predictor_path, predictor_io
        write(predictor_io, "function (theta, columns, row_index)\n")
        write(predictor_io, "    @inbounds return theta[1] * exp(-theta[2] * columns[row_index, 1]) + theta[3]\n")
        write(predictor_io, "end\n")
        close(predictor_io)
        mktemp() do likelihood_path, likelihood_io
            write(likelihood_io, "function (theta, columns, y)\n")
            write(likelihood_io, "    result = zero(eltype(theta))\n")
            write(likelihood_io, "    @inbounds for row in eachindex(y)\n")
            write(likelihood_io, "        result += logpdf(Normal(theta[1] * exp(-theta[2] * columns[row, 1]) + theta[3], theta[4]), y[row])\n")
            write(likelihood_io, "    end\n")
            write(likelihood_io, "    return result\nend\n")
            close(likelihood_io)
            exchange = Dict(
                "predictorKernelPath" => predictor_path,
                "likelihoodKernelPath" => likelihood_path,
                "predictorColumns" => ["time"],
            )

            compiled = bayes_load_predictor(exchange, table, 3, "task")
            theta = [2.0, 0.5, 1.0, 0.25]
            expected = [3.0, 2.0 * exp(-0.5) + 1.0, 2.0 * exp(-1.0) + 1.0]
            @test compiled.columns == reshape([0.0, 1.0, 2.0], 3, 1)
            @test [compiled.predictor(theta, compiled.columns, row) for row in 1:3] ≈ expected
            @test compiled.likelihood(theta, compiled.columns, expected) ≈
                sum(logpdf(Normal(value, theta[4]), value) for value in expected)
        end
    end
end

@testset "Bayesian capability boundary" begin
    unsupported = UnsupportedBayesCapability("test model")
    @test sprint(showerror, unsupported) == "unsupported capability: test model"
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

@testset "NUTS tree depth diagnostics" begin
    chain = Chains(
        reshape([6.0, 7.0, 8.0], 3, 1, 1),
        [:tree_depth],
        Dict(:internals => [:tree_depth]),
    )
    @test bayes_max_treedepth_hits(chain, 7) == 2

    parameter_chain = Chains(reshape([1.0, 2.0], 2, 1, 1), [:theta])
    @test bayes_max_treedepth_hits(parameter_chain, 7) === nothing
end

@testset "Bayes diagnostic warnings are structured" begin
    warnings = bayes_diagnostic_warnings([
        Dict(
            "parameter" => "beta",
            "rhat" => 1.23456789,
            "essBulk" => 20.12345,
            "essTail" => 30.67891,
        ),
    ], 1_000)

    @test length(warnings) == 3
    @test Set(warning["metric"] for warning in warnings) == Set(["rhat", "ess_bulk", "ess_tail"])
    @test all(Set(keys(warning)) == Set(["code", "metric", "value", "threshold", "parameter"]) for warning in warnings)
    @test all(!haskey(warning, "message") for warning in warnings)

    by_metric = Dict(warning["metric"] => warning for warning in warnings)
    @test by_metric["rhat"]["code"] == "rhat_too_high"
    @test by_metric["rhat"]["value"] == 1.23456789
    @test by_metric["rhat"]["threshold"] == 1.01
    @test by_metric["ess_bulk"]["code"] == "ess_too_low"
    @test by_metric["ess_bulk"]["value"] == 20.12345
    @test by_metric["ess_bulk"]["threshold"] == 100.0
    @test by_metric["ess_tail"]["code"] == "ess_too_low"
    @test by_metric["ess_tail"]["value"] == 30.67891
    @test by_metric["ess_tail"]["threshold"] == 100.0
    @test all(warning["parameter"] == "beta" for warning in warnings)
end
