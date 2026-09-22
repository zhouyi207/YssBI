export interface HistogramBin {
  label: string;
  count: number;
}

export interface CategoryCount {
  label: string;
  value: number;
}

export interface NumericDistribution {
  columnName: string;
  kind: "numeric";
  bins: HistogramBin[];
}

export interface StringDistribution {
  columnName: string;
  kind: "string";
  categories: CategoryCount[];
  otherCount: number;
}

export type ColumnDistribution = NumericDistribution | StringDistribution;

export interface EditState {
  canUndo: boolean;
  canRedo: boolean;
  isModified: boolean;
  undoCount: number;
  redoCount: number;
}
