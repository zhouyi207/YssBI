// src/features/core/sync/handlers/BaseEventHandler.ts

import { EventHandler, EventCallbacks } from '../types';
import { logger } from '@/utils/appLogger';

export abstract class BaseEventHandler<T = unknown> implements EventHandler<T> {
    abstract eventType: string;
    
    constructor(protected callbacks?: EventCallbacks) {}
    
    abstract handle(payload: T, callbacks?: EventCallbacks): void;
    
    protected log(message: string, ...args: any[]): void {
        logger.sys.debug(message + (args.length ? ' ' + args.map(String).join(' ') : ''), this.constructor.name);
    }
    
    protected error(message: string, ...args: any[]): void {
        logger.sys.error(message + (args.length ? ' ' + args.map(String).join(' ') : ''), this.constructor.name);
    }
}
