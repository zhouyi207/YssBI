// src/features/core/sync/registry/EventRegistry.ts

import { EventHandler, RawBackendEvent } from '../types';
import { parseEvent, isValidEventType } from '../utils/eventParser';
import { logger } from '@/utils/appLogger';

/**
 * 事件注册中心
 * 管理所有事件处理器的注册和分发
 */
export class EventRegistry {
    private handlers = new Map<string, EventHandler>();

    constructor(handlers: EventHandler[] = []) {
        handlers.forEach(handler => this.register(handler));
    }

    /**
     * 注册事件处理器
     */
    register(handler: EventHandler): void {
        if (this.handlers.has(handler.eventType)) {
            logger.sys.warn(`Handler for '${handler.eventType}' already registered, overwriting...`, 'EventRegistry');
        }
        this.handlers.set(handler.eventType, handler);
    }

    /**
     * 注销事件处理器
     */
    unregister(eventType: string): void {
        this.handlers.delete(eventType);
    }

    /**
     * 分发事件到对应的处理器
     */
    dispatch(event: RawBackendEvent): void {
        const parsed = parseEvent(event);
        
        if (!isValidEventType(parsed.type)) {
            logger.sys.warn(`Unknown event type: ${parsed.type}`, 'EventRegistry');
            return;
        }

        const handler = this.handlers.get(parsed.type);
        
        if (!handler) {
            logger.sys.warn(`No handler registered for event type: ${parsed.type}`, 'EventRegistry');
            return;
        }

        try {
            handler.handle(parsed.payload);
        } catch (error) {
            logger.sys.error(`Error handling event '${parsed.type}': ${error instanceof Error ? error.message : String(error)}`, 'EventRegistry');
        }
    }

    /**
     * 获取已注册的事件类型列表
     */
    getRegisteredTypes(): string[] {
        return Array.from(this.handlers.keys());
    }

    /**
     * 清空所有处理器
     */
    clear(): void {
        this.handlers.clear();
    }
}
