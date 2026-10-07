export const PROJECT_TREE_CATEGORY_IDS = {
  eventGraphs: "project.eventGraphs",
  functionGraphs: "project.functionGraphs",
  charts: "project.charts",
  minds: "project.minds",
  docs: "project.docs",
  data: "project.data",
} as const;

export type ProjectTreeCategoryId =
  (typeof PROJECT_TREE_CATEGORY_IDS)[keyof typeof PROJECT_TREE_CATEGORY_IDS];
