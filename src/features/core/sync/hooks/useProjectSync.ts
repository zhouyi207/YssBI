// src/features/core/sync/hooks/useProjectSync.ts

import { useEffect, useRef } from 'react';
import { ProjectListener } from '../listeners/ProjectListener';
import { SingletonManager } from '../utils/singletonManager';
import { EventCallbacks } from '../types';

const LISTENER_KEY = 'project-listener';

/**
 * 项目同步 Hook
 * 使用单例模式确保全局只有一个监听器
 */
export function useProjectSync(callbacks?: EventCallbacks) {
    const callbacksRef = useRef(callbacks);
    const isSetupRef = useRef(false);

    // 更新 callbacks ref，但不触发重新设置监听器
    useEffect(() => {
        callbacksRef.current = callbacks;
    }, [callbacks]);

    useEffect(() => {
        // 防止重复设置
        if (isSetupRef.current) {
            return;
        }

        isSetupRef.current = true;
        let listener: ProjectListener;

        const setup = async () => {
            listener = await SingletonManager.getInstance(
                LISTENER_KEY,
                async () => {
                    console.log('[useProjectSync] Creating new listener instance');
                    const newListener = new ProjectListener(callbacksRef.current);
                    await newListener.start();
                    return newListener;
                }
            );

            // 如果监听器已存在，更新回调
            if (callbacksRef.current && SingletonManager.getRefCount(LISTENER_KEY) > 1) {
                console.log('[useProjectSync] Updating callbacks for existing listener');
                listener.updateCallbacks(callbacksRef.current);
            }
        };

        setup();

        return () => {
            isSetupRef.current = false;
            SingletonManager.decrementRef(LISTENER_KEY, (instance: ProjectListener) => {
                console.log('[useProjectSync] Cleaning up listener');
                instance.stop();
            });
        };
    }, []); // 空依赖数组，只在挂载时运行一次
}
