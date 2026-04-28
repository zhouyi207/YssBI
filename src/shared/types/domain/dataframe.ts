export interface NumericColumnStats {
  columnName: string;
  columnType: string;
  kind: "numeric";
  count: number;
  nullCount: number;
  min: number | null;
  max: number | null;
  mean: number | null;
  median: number | null;
  std: number | null;
  variance: number | null;
}

export interface StringColumnStats {
  columnName: string;
  columnType: string;
  kind: "string";
  count: number;
  nullCount: number;
  emptyCount: number;
  validRatio: number;
  unique: number;
  mode: string | null;
  modeCount: number;
}

export type ColumnStats = NumericColumnStats | StringColumnStats;

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

export interface SizeShape {
  nRows: number;
  nColumns: number;
  memorySize: number;
  duplicatedRows: number;
}

export interface SchemaOverview {
  numericCols: number;
  categoricalCols: number;
  stringCols: number;
  datetimeCols: number;
  boolCols: number;
}

export interface DataCompleteness {
  totalNulls: number;
  nullRatio: number;
  colsWithNulls: number;
  rowsWithNulls: number;
}

export interface DatasetOverview {
  sizeShape: SizeShape;
  schemaOverview: SchemaOverview;
  dataCompleteness: DataCompleteness;
}

export interface EditState {
  canUndo: boolean;
  canRedo: boolean;
  isModified: boolean;
  undoCount: number;
  redoCount: number;
}

export interface EditingCell {
  row: number;
  col: number;
}
