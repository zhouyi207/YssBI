// src/features/core/sync/utils/singletonManager.ts

/**
 * 单例监听器管理器
 * 确保全局只有一个事件监听器实例
 */
export class SingletonManager {
    private static instances = new Map<string, any>();
    private static refCounts = new Map<string, number>();

    /**
     * 获取或创建单例
     */
    static getInstance<T>(
        key: string,
        factory: () => Promise<T>
    ): Promise<T> {
        if (this.instances.has(key)) {
            this.incrementRef(key);
            return Promise.resolve(this.instances.get(key));
        }

        return factory().then(instance => {
            this.instances.set(key, instance);
            this.refCounts.set(key, 1);
            return instance;
        });
    }

    /**
     * 增加引用计数
     */
    static incrementRef(key: string): void {
        const count = this.refCounts.get(key) || 0;
        this.refCounts.set(key, count + 1);
    }

    /**
     * 减少引用计数，当计数为 0 时清理实例
     */
    static decrementRef(key: string, cleanup?: (instance: any) => void): void {
        const count = this.refCounts.get(key) || 0;
        
        if (count <= 1) {
            const instance = this.instances.get(key);
            if (cleanup && instance) {
                cleanup(instance);
            }
            this.instances.delete(key);
            this.refCounts.delete(key);
        } else {
            this.refCounts.set(key, count - 1);
        }
    }

    /**
     * 检查实例是否存在
     */
    static has(key: string): boolean {
        return this.instances.has(key);
    }

    /**
     * 获取引用计数
     */
    static getRefCount(key: string): number {
        return this.refCounts.get(key) || 0;
    }
}
