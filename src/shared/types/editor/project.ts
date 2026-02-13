

export interface ProjectData {
  /** 全局变量 */
  globalVariables: Record<string, Variable>;
  events: Record<string, GraphData>;
  functions: Record<string, GraphData>;
  macros: Record<string, GraphData>;
  /** 数据帧 */
  dataframes: Record<string, DataFrameData>;
  metadata: {
    exportTime: string;
    appVersion: string;
  };
}
