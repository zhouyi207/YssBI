import type {
  BayesDatasetSelectionDTO,
  ParameterConstraintDTO,
  PriorSpecDTO,
} from "@/shared/types/bayes";

export function numericColumns(
  columns: BayesDatasetSelectionDTO["columns"],
): BayesDatasetSelectionDTO["columns"] {
  return columns.filter((column) => column.dtype === "number" || column.dtype === "integer");
}

export function isPriorCompatibleWithConstraint(
  distribution: PriorSpecDTO["distribution"],
  constraint: ParameterConstraintDTO,
): boolean {
  switch (constraint.type) {
    case "real":
      return ["normal", "student_t", "cauchy", "uniform"].includes(distribution);
    case "positive":
      return ["log_normal", "gamma", "exponential", "half_normal"].includes(distribution);
    case "unit":
      return ["beta", "uniform"].includes(distribution);
    case "bounded":
      return distribution === "uniform";
  }
}

export function priorArgumentCount(distribution: PriorSpecDTO["distribution"]): number {
  if (distribution === "student_t") return 3;
  if (distribution === "exponential" || distribution === "half_normal") return 1;
  return 2;
}

export function defaultPriorArgs(distribution: PriorSpecDTO["distribution"]): number[] {
  switch (distribution) {
    case "normal":
      return [0, 10];
    case "log_normal":
      return [0, 1];
    case "uniform":
      return [0, 1];
    case "beta":
      return [2, 2];
    case "gamma":
      return [2, 1];
    case "exponential":
      return [1];
    case "student_t":
      return [3, 0, 10];
    case "cauchy":
      return [0, 2.5];
    case "half_normal":
      return [5];
  }
}

export function priorFromParts(
  distribution: PriorSpecDTO["distribution"],
  args: string[],
): PriorSpecDTO {
  const values = Array.from({ length: priorArgumentCount(distribution) }, (_, index) =>
    Number(args[index]),
  );
  const fallback = defaultPriorArgs(distribution);
  const safe = values.map((value, index) => (Number.isFinite(value) ? value : fallback[index]));
  switch (distribution) {
    case "normal":
    case "log_normal":
    case "uniform":
    case "beta":
    case "gamma":
    case "cauchy":
      return {
        distribution,
        args: [safe[0] ?? fallback[0], safe[1] ?? fallback[1]],
      } as PriorSpecDTO;
    case "student_t":
      return { distribution, args: [safe[0] ?? 3, safe[1] ?? 0, safe[2] ?? 10] };
    case "exponential":
    case "half_normal":
      return { distribution, args: [safe[0] ?? fallback[0]] } as PriorSpecDTO;
  }
}
