import type { ParameterConstraintDTO, ParameterSpecDTO, PriorSpecDTO } from "@/shared/types/bayes";

export function defaultPriorForConstraint(
  constraint: ParameterConstraintDTO,
  name?: string,
): PriorSpecDTO {
  if (name === "sigma") {
    return { distribution: "exponential", args: [1] };
  }
  switch (constraint.type) {
    case "positive":
      return { distribution: "exponential", args: [1] };
    case "unit":
      return { distribution: "beta", args: [2, 2] };
    case "bounded":
      return { distribution: "uniform", args: [constraint.lower, constraint.upper] };
    case "real":
    default:
      return { distribution: "normal", args: [0, 10] };
  }
}

export function defaultConstraintForParameter(name: string): ParameterConstraintDTO {
  return name === "sigma" ? { type: "positive" } : { type: "real" };
}

export function createDefaultParameter(name: string): ParameterSpecDTO {
  const constraint = defaultConstraintForParameter(name);
  return {
    name,
    constraint,
    prior: defaultPriorForConstraint(constraint, name),
  };
}

export function formatPrior(prior: PriorSpecDTO): string {
  const name = prior.distribution
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
  return `${name}(${prior.args.join(", ")})`;
}
