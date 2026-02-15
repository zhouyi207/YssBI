// src/features/core/sync/registry/EventRegistry.ts

import { EventHandler, EventCallbacks } from '../types';
import { parseEvent, isValidEventType } from '../utils/eventParser';

/**
 * 事件注册中心
 * 管理所有事件处理器的注册和分发
 */
export class EventRegistry {
    private handlers = new Map<string, EventHandler>();
    private callbacks?: EventCallbacks;

    constructor(handlers: EventHandler[] = [], callbacks?: EventCallbacks) {
        this.callbacks = callbacks;
        handlers.forEach(handler => this.register(handler));
    }

    /**
     * 注册事件处理器
     */
    register(handler: EventHandler): void {
        if (this.handlers.has(handler.eventType)) {
            console.warn(`[EventRegistry] Handler for '${handler.eventType}' already registered, overwriting...`);
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
    dispatch(event: any): void {
        const parsed = parseEvent(event);
        
        if (!isValidEventType(parsed.type)) {
            console.warn(`[EventRegistry] Unknown event type: ${parsed.type}`);
            return;
        }

        const handler = this.handlers.get(parsed.type);
        
        if (!handler) {
            console.warn(`[EventRegistry] No handler registered for event type: ${parsed.type}`);
            return;
        }

        try {
            handler.handle(parsed.payload, this.callbacks);
        } catch (error) {
            console.error(`[EventRegistry] Error handling event '${parsed.type}':`, error);
        }
    }

    /**
     * 更新回调函数
     */
    updateCallbacks(callbacks: EventCallbacks): void {
        this.callbacks = callbacks;
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
