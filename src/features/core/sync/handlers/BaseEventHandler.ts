// src/features/core/sync/handlers/BaseEventHandler.ts

import { EventHandler, EventCallbacks } from '../types';

export abstract class BaseEventHandler<T = any> implements EventHandler<T> {
    abstract eventType: string;
    
    constructor(protected callbacks?: EventCallbacks) {}
    
    abstract handle(payload: T, callbacks?: EventCallbacks): void;
    
    protected log(message: string, ...args: any[]): void {
        console.log(`[${this.constructor.name}] ${message}`, ...args);
    }
    
    protected error(message: string, ...args: any[]): void {
        console.error(`[${this.constructor.name}] ${message}`, ...args);
    }
}
