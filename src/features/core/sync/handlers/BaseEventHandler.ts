// src/features/core/sync/handlers/BaseEventHandler.ts

import { EventHandler } from '../types';
import { logger } from '@/utils/appLogger';

export abstract class BaseEventHandler<T = unknown> implements EventHandler<T> {
    abstract eventType: string;
    
    abstract handle(payload: T): void;
    
    protected log(message: string, ...args: any[]): void {
        logger.sys.debug(message + (args.length ? ' ' + args.map(String).join(' ') : ''), this.constructor.name);
    }
    
    protected error(message: string, ...args: any[]): void {
        logger.sys.error(message + (args.length ? ' ' + args.map(String).join(' ') : ''), this.constructor.name);
    }
}
