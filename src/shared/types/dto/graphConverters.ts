/**
 * Graph / Connection DTO 转换
 */

import type { ConnectionItemDTO } from "./graph";
import type { ConnectionData } from "../store/graph";
import type { ProjectData } from "../domain";

/** 将 ConnectionData 转为 ConnectionItemDTO */
export function connectionDataToItem(conn: Pick<ConnectionData, "from" | "to">): ConnectionItemDTO {
  return { fromPin: conn.from, toPin: conn.to };
}

/** 将 ConnectionData 列表转为 ConnectionItemDTO */
export function connectionDataToItems(
  conns: ReadonlyArray<Pick<ConnectionData, "from" | "to">>,
): ConnectionItemDTO[] {
  return conns.map(connectionDataToItem);
}

/** 深度克隆 DTO */
export function cloneDTO<T>(dto: T): T {
  return JSON.parse(JSON.stringify(dto));
}

/** 合并 ProjectData */
export function mergeProjectData(base: ProjectData, updates: Partial<ProjectData>): ProjectData {
  return {
    variables: { ...base.variables, ...updates.variables },
    graphs: { ...base.graphs, ...updates.graphs },
    databases: { ...base.databases, ...updates.databases },
    metadata: updates.metadata || base.metadata,
  };
}
