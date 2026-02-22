// src/features/core/sync/listeners/ProjectListener.ts

import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { EventRegistry } from '../registry/EventRegistry';
import { createEventHandlers } from '../handlers';
import { EventCallbacks } from '../types';
import { logger } from '@/utils/appLogger';

/**
 * 项目事件监听器
 * 监听 'project-event' 并分发到事件注册中心
 */
export class ProjectListener {
    private unlisten: UnlistenFn | null = null;
    private registry: EventRegistry;

    constructor(callbacks?: EventCallbacks) {
        const handlers = createEventHandlers();
        this.registry = new EventRegistry(handlers, callbacks);
    }

    /**
     * 启动监听
     */
    async start(): Promise<void> {
        if (this.unlisten) {
            logger.sys.debug('Already listening', 'ProjectListener');
            return;
        }

        logger.sys.debug('Starting project event listener...', 'ProjectListener');

        this.unlisten = await listen('project-event', (event) => {
            logger.sys.trace('Received event: ' + JSON.stringify(event.payload), 'ProjectListener');
            this.registry.dispatch(event.payload);
        });

        logger.sys.info('Project event listener started', 'ProjectListener');
    }

    /**
     * 停止监听
     */
    stop(): void {
        if (this.unlisten) {
            this.unlisten();
            this.unlisten = null;
            logger.sys.info('Project event listener stopped', 'ProjectListener');
        }
    }

    /**
     * 更新回调函数
     */
    updateCallbacks(callbacks: EventCallbacks): void {
        this.registry.updateCallbacks(callbacks);
    }

    /**
     * 检查是否正在监听
     */
    isListening(): boolean {
        return this.unlisten !== null;
    }
}
