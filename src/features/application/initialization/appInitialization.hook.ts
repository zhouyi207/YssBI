import { useState, useEffect, useRef } from 'react';
import { InitializationState } from './appInitialization.type';
import { LoadStatus } from '@/shared/types/ui';
import { useSchema } from '@/features/core/schema';
import { useNodeRegistry } from '@/features/core/nodeRegister';
import { initProjectSync } from '@/features/core/project';
import { useProjectSync } from '../../core/project/projectSync';


export function useAppInitialization(): InitializationState {
    const [state, setState] = useState<InitializationState>({
        status: LoadStatus.Idle,
        error: null,
    });

    // 使用 ref 防止重复初始化项目同步
    const hasRestoredProjectRef = useRef(false);

    const { status: schemaStatus, error: schemaError } = useSchema();
    const { status: registryStatus, error: registryError } = useNodeRegistry();

    const isSchemaReady = schemaStatus === LoadStatus.Ready;
    const isRegistryReady = registryStatus === LoadStatus.Ready;

    // 监听依赖状态变化
    useEffect(() => {
        // 如果已经同步过项目，直接标记完成
        if (hasRestoredProjectRef.current) {
            setState({ status: LoadStatus.Ready, error: null });
            return;
        }

        // 如果已经出错，不继续
        if (state.error) return;

        // 检查是否有错误
        if (schemaError) {
            console.error('[AppInit] ✗ Initialization failed: Schema error ', schemaError);
            setState({ status: LoadStatus.Error, error: `Schema: ${schemaError}` });
            return;
        }

        if (registryError) {
            console.error('[AppInit] ✗ Initialization failed: NodeRegistry error ', registryError);
            setState({ status: LoadStatus.Error, error: `NodeRegistry: ${registryError}` });
            return;
        }

        // 如果 Schema 或 Registry 还未准备好，等待
        if (!isSchemaReady || !isRegistryReady) {
            setState({ status: LoadStatus.Loading, error: null });
            return;
        }

        // 同步项目状态
        const syncProject = async () => {
            try {
                await initProjectSync();
                hasRestoredProjectRef.current = true;
                setState({ status: LoadStatus.Ready, error: null });
            } catch (error) {
                const errorMessage = error instanceof Error ? error.message : String(error);
                console.error('[AppInit] ✗ Failed to sync project:', errorMessage);
                setState({
                    status: LoadStatus.Error,
                    error: `Project sync: ${errorMessage}`,
                });
            }
        };

        syncProject();
    }, [schemaStatus, registryStatus]);

    useProjectSync({ enabled: true });

    return state;
}
