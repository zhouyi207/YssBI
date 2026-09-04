import { Channel } from "@tauri-apps/api/core";

import { invokeCommand } from "@/services/ipc";
import type {
  GraphProjectionChannelEventDto,
  GraphProjectionSnapshotDto,
  GraphProjectionSubscriptionDto,
} from "@/shared/types/dto/graphProjectionChannel";
import {
  parseGraphProjectionChannelEventDto,
  parseGraphProjectionSnapshotDto,
  parseGraphProjectionSubscriptionDto,
} from "@/shared/types/dto/graphProjectionChannelWireParser";

export interface GraphProjectionSubscription {
  readonly snapshot: GraphProjectionSnapshotDto;
  activate(): void;
  unsubscribe(): Promise<void>;
}

export class GraphProjectionChannelService {
  static async subscribe(
    projectInstanceId: string,
    onEvent: (event: GraphProjectionChannelEventDto) => void,
    onMalformedEvent: (error: unknown) => void,
  ): Promise<GraphProjectionSubscription> {
    const pending: GraphProjectionChannelEventDto[] = [];
    let active = false;
    let closed = false;
    const channel = new Channel<unknown>();
    channel.onmessage = (value) => {
      if (closed) return;
      try {
        const event = parseGraphProjectionChannelEventDto(value);
        if (active) onEvent(event);
        else pending.push(event);
      } catch (error) {
        onMalformedEvent(error);
      }
    };

    let subscription: GraphProjectionSubscriptionDto;
    try {
      subscription = parseGraphProjectionSubscriptionDto(
        await invokeCommand("subscribe_graph_projections", {
          projectInstanceId,
          onEvents: channel,
        }),
      );
    } catch (error) {
      closed = true;
      channel.onmessage = () => undefined;
      throw error;
    }
    if (subscription.snapshot.projectInstanceId !== projectInstanceId) {
      closed = true;
      channel.onmessage = () => undefined;
      await GraphProjectionChannelService.unsubscribe(subscription.subscriptionId).catch(
        () => undefined,
      );
      throw new Error("Graph Projection subscription targets another project");
    }

    return {
      snapshot: subscription.snapshot,
      activate: () => {
        if (closed || active) return;
        active = true;
        for (const event of pending.splice(0)) onEvent(event);
      },
      unsubscribe: async () => {
        if (closed) return;
        closed = true;
        pending.length = 0;
        channel.onmessage = () => undefined;
        await GraphProjectionChannelService.unsubscribe(subscription.subscriptionId);
      },
    };
  }

  static async snapshot(projectInstanceId: string): Promise<GraphProjectionSnapshotDto> {
    return parseGraphProjectionSnapshotDto(
      await invokeCommand("get_graph_projection_snapshot", { projectInstanceId }),
    );
  }

  static async unsubscribe(subscriptionId: string): Promise<void> {
    await invokeCommand("unsubscribe_graph_projections", { subscriptionId });
  }
}
