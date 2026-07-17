import type { LikelihoodSpecDTO, ParameterSpecDTO } from '@/shared/types/bayes';
import { createDefaultParameter } from './priorDefaults';

export interface ParameterMergeResult {
  parameters: ParameterSpecDTO[];
  unusedParameterNames: string[];
}

export function mergeInferredParameters(
  existing: readonly ParameterSpecDTO[],
  inferredNames: readonly string[],
  likelihood: LikelihoodSpecDTO,
): ParameterMergeResult {
  const required = new Set(inferredNames);
  for (const name of likelihoodParameterNames(likelihood)) {
    required.add(name);
  }

  const existingByName = new Map(existing.map(parameter => [parameter.name, parameter]));
  const parameters = Array.from(required)
    .sort()
    .map(name => existingByName.get(name) ?? createDefaultParameter(name));
  const unusedParameterNames = existing
    .map(parameter => parameter.name)
    .filter(name => !required.has(name))
    .sort();

  return { parameters, unusedParameterNames };
}

export function likelihoodParameterNames(likelihood: LikelihoodSpecDTO): string[] {
  switch (likelihood.type) {
    case 'normal':
      return [likelihood.sigma.parameter];
    case 'bernoulli_logit':
    case 'poisson_log':
      return [];
  }
}
