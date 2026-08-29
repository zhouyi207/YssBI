/** Product metadata and external links (single source for Help / About). */
export const APP_DISPLAY_NAME = "YssBI";
export const APP_VERSION = "0.3.0";

const GITHUB_REPO = "https://github.com/zhouyi207/YssBI";

export const APP_LINKS = {
  documentation: `${GITHUB_REPO}/blob/shadcn/README.md`,
  releaseNotes: `${GITHUB_REPO}/releases`,
  repository: GITHUB_REPO,
  reportIssue: `${GITHUB_REPO}/issues`,
} as const;
