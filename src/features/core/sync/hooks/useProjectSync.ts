// src/features/core/sync/hooks/useProjectSync.ts

import { useEffect, useRef } from 'react';
import { ProjectListener } from '../listeners/ProjectListener';
import { SingletonManager } from '../utils/singletonManager';
import { useEditor } from '@/features/application/editor/core/hooks/useEditor';
import { useMemo } from "react";

const LISTENER_KEY = 'project-listener';

/**
 * 项目同步 Hook
 * 使用单例模式确保全局只有一个监听器
 */
export function useProjectSync() {

    const editor = useEditor();

    // 使用 useMemo 稳定回调引用，避免重复创建监听器
    const projectSyncCallbacks = useMemo(() => ({
        onEventCreated: editor.handleEventCreated,
        onEventCreatedFailed: editor.handleEventCreatedFailed,
        onFunctionCreated: editor.handleFunctionCreated,
        onFunctionCreatedFailed: editor.handleFunctionCreatedFailed,
        onMacroCreated: editor.handleMacroCreated,
        onMacroCreatedFailed: editor.handleMacroCreatedFailed,
        onNodeCreated: editor.handleNodeCreated,
        onNodeDeleted: editor.handleNodeDeleted,
    }), [
        editor.handleEventCreated,
        editor.handleEventCreatedFailed,
        editor.handleFunctionCreated,
        editor.handleFunctionCreatedFailed,
        editor.handleMacroCreated,
        editor.handleMacroCreatedFailed,
        editor.handleNodeCreated,
        editor.handleNodeDeleted,
    ]);

    const isSetupRef = useRef(false);



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
                    const newListener = new ProjectListener(projectSyncCallbacks);
                    await newListener.start();
                    return newListener;
                }
            );

            // 如果监听器已存在，更新回调
            if (projectSyncCallbacks && SingletonManager.getRefCount(LISTENER_KEY) > 1) {
                console.log('[useProjectSync] Updating callbacks for existing listener');
                listener.updateCallbacks(projectSyncCallbacks);
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
