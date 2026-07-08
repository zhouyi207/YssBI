import { logger } from "@/utils/appLogger";
import { deactivateInactiveGraphPath } from "./deactivateInactiveGraphPath";

/** Release frontend/backend graph cache when the path is fully closed and inactive. */
export function releaseGraphCacheIfClosed(graphPath: string): void {
  void deactivateInactiveGraphPath(graphPath).catch((error) => {
    logger.graph.warn(
      `Failed to release graph cache '${graphPath}': ${error instanceof Error ? error.message : String(error)}`,
      "releaseGraphCache"
    );
  });
}
