export type ParameterConstraintDTO =
  | { type: "real" }
  | { type: "positive" }
  | { type: "unit" }
  | { type: "bounded"; lower: number; upper: number; includeLower: boolean; includeUpper: boolean };

export type PriorSpecDTO =
  | { distribution: "normal"; args: [number, number] }
  | { distribution: "log_normal"; args: [number, number] }
  | { distribution: "uniform"; args: [number, number] }
  | { distribution: "beta"; args: [number, number] }
  | { distribution: "gamma"; args: [number, number] }
  | { distribution: "exponential"; args: [number] }
  | { distribution: "student_t"; args: [number, number, number] }
  | { distribution: "cauchy"; args: [number, number] }
  | { distribution: "half_normal"; args: [number] };

export interface ParameterSpecDTO {
  name: string;
  constraint: ParameterConstraintDTO;
  prior: PriorSpecDTO;
}
