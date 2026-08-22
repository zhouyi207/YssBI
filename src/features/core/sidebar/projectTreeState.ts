export const PROJECT_TREE_CATEGORY_IDS = {
  events: 'project.events',
  functions: 'project.functions',
  worksheets: 'project.worksheets',
  activeGraphVariables: 'project.activeGraphVariables',
  globalVariables: 'project.globalVariables',
} as const;

export type ProjectTreeCategoryId = (
  typeof PROJECT_TREE_CATEGORY_IDS[keyof typeof PROJECT_TREE_CATEGORY_IDS]
);

export const PROJECT_TREE_EXPANSION_DEFAULTS = {
  [PROJECT_TREE_CATEGORY_IDS.events]: true,
  [PROJECT_TREE_CATEGORY_IDS.functions]: false,
  [PROJECT_TREE_CATEGORY_IDS.worksheets]: true,
  [PROJECT_TREE_CATEGORY_IDS.activeGraphVariables]: true,
  [PROJECT_TREE_CATEGORY_IDS.globalVariables]: false,
} as const;
