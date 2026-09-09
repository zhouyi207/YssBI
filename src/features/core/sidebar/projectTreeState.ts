export const PROJECT_TREE_CATEGORY_IDS = {
  events: "project.events",
  functions: "project.functions",
  charts: "project.charts",
  data: "project.data",
} as const;

export type ProjectTreeCategoryId =
  (typeof PROJECT_TREE_CATEGORY_IDS)[keyof typeof PROJECT_TREE_CATEGORY_IDS];
