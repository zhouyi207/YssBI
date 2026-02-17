/**
 * 后端 API 响应类型
 * 与 invoke() 返回值对应
 */

import type { GraphInstanceDTO } from './graph';
import type { Variable } from '../domain/variable';
import type { DatabaseDecl } from '../domain/database';

/** 项目数据 DTO（get_project_data / load_project_to_state 返回值） */
export interface ProjectDataDTO {
  variables: Record<string, Variable>;
  graphs: Record<string, GraphInstanceDTO>;
  databases: Record<string, DatabaseDecl>;
  metadata: { exportTime: string; appVersion: string };
}
