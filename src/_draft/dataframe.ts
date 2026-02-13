
/** DataFrame 统计信息 */
export interface DataFrameStats {
  /** 行数 */
  row_count: number;
  /** 列数 */
  column_count: number;
  /** 列信息 */
  columns: ColumnInfo[];
}

/** 列信息 */
export interface ColumnInfo {
  /** 列名 */
  name: string;
  /** 数据类型 */
  dtype: string;
  /** 非空值数量 */
  non_null_count: number;
  /** 唯一值数量 */
  unique_count?: number;
}


export interface DataFrameColumn {
  name: string;
  type: string;
}

export interface DataFrameData {
  id: string;
  name: string;
  columns: DataFrameColumn[];
  rows: any[][];
  rowCount: number;
  columnCount: number;
  sourcePath?: string;
}