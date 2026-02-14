import { Graph } from "./graph";
import { Variable } from "./variable";

export interface ProjectMetadata {
  exportTime: string;
  appVersion: string;
}

export interface ProjectData {
  variables: Record<string, Variable>;
  graphs: Record<string, Graph>;
  databases: Record<string, any>;
  metadata: ProjectMetadata;
}

// DTO 类型与 ProjectData 一致
export type ProjectDataDTO = ProjectData;

// 前后端转换辅助函数
export const ProjectDataConverter = {
  toDTO(data: ProjectData): ProjectDataDTO {
    return data;
  },

  fromDTO(dto: ProjectDataDTO): ProjectData {
    return dto;
  },
};