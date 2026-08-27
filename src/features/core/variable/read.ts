export interface VariableReadSnapshot {
  readonly id: string;
  readonly name: string;
  readonly revision: number;
  readonly dataType: string;
  readonly value: unknown;
}

export interface VariableReadCapability {
  readonly getVariable: (id: string) => VariableReadSnapshot | null;
  readonly listVariables: () => readonly VariableReadSnapshot[];
}
