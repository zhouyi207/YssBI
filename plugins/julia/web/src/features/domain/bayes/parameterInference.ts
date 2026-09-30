import type { LikelihoodSpecDTO, ParameterSpecDTO } from "@/shared/types/bayes";
import { createDefaultParameter } from "./priorDefaults";

export function mergeInferredParameters(
  existing: readonly ParameterSpecDTO[],
  inferredNames: readonly string[],
  likelihood: LikelihoodSpecDTO,
): ParameterSpecDTO[] {
  const required = new Set(inferredNames);
  for (const name of likelihoodParameterNames(likelihood)) {
    required.add(name);
  }

  const existingByName = new Map(existing.map((parameter) => [parameter.name, parameter]));
  return Array.from(required)
    .sort()
    .map((name) => existingByName.get(name) ?? createDefaultParameter(name));
}

export function likelihoodParameterNames(likelihood: LikelihoodSpecDTO): string[] {
  switch (likelihood.type) {
    case "normal":
      return [likelihood.sigma.parameter];
    case "bernoulli_logit":
    case "poisson_log":
      return [];
  }
}
