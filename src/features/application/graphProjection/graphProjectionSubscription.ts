import { useEffect } from "react";

import { logger } from "@/features/application/observability/appLogger";
import { GraphProjectionChannelService } from "@/services/nodeSystem/graphProjectionChannelService";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import {
  acceptGraphProjectionEvent,
  acceptGraphProjectionSnapshot,
  recoverCurrentGraphProjections,
} from "./graphProjectionCoordinator";
import {
  requestGraphProjectionReconnect,
  subscribeGraphProjectionReconnect,
} from "./graphProjectionConnection";

export function useGraphProjectionSubscription(projectInstanceId: string | null): void {
  useEffect(() => {
    if (!projectInstanceId) return;
    let cancelled = false;
    let unsubscribe: (() => Promise<void>) | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let connectionGeneration = 0;

    const connect = () => {
      const generation = ++connectionGeneration;
      void GraphProjectionChannelService.subscribe(
        projectInstanceId,
        (event) => {
          if (!cancelled) acceptGraphProjectionEvent(event);
        },
        (error) => {
          if (cancelled) return;
          logger.graph.error(
            `Malformed Graph Projection event: ${formatErrorMessage(error)}`,
            "GraphProjectionCoordinator",
          );
          requestGraphProjectionReconnect();
          void recoverCurrentGraphProjections(projectInstanceId).catch((recoveryError) => {
            if (cancelled) return;
            logger.graph.error(
              `Graph Projection snapshot recovery failed: ${formatErrorMessage(recoveryError)}`,
              "GraphProjectionCoordinator",
            );
          });
        },
      )
        .then((subscription) => {
          if (cancelled || generation !== connectionGeneration) {
            void subscription.unsubscribe().catch(() => undefined);
            return;
          }
          acceptGraphProjectionSnapshot(subscription.snapshot);
          subscription.activate();
          unsubscribe = subscription.unsubscribe;
        })
        .catch((error) => {
          if (cancelled || generation !== connectionGeneration) return;
          logger.graph.error(
            `Graph Projection subscription failed: ${formatErrorMessage(error)}`,
            "GraphProjectionCoordinator",
          );
          reconnectTimer = setTimeout(connect, 1_000);
        });
    };
    const reconnect = () => {
      if (cancelled) return;
      connectionGeneration += 1;
      if (reconnectTimer) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
      const close = unsubscribe;
      unsubscribe = null;
      if (close) void close().catch(() => undefined);
      connect();
    };
    const unsubscribeReconnect = subscribeGraphProjectionReconnect(reconnect);
    connect();

    return () => {
      cancelled = true;
      connectionGeneration += 1;
      unsubscribeReconnect();
      if (reconnectTimer) clearTimeout(reconnectTimer);
      if (unsubscribe) void unsubscribe().catch(() => undefined);
    };
  }, [projectInstanceId]);
}
