import { useState, useEffect, useRef } from 'react';
import { InitializationState } from './appInitialization.type';
import { LoadStatus } from '@/shared/types/ui';
import { useSchemaStore } from '@/features/core/schema';
import { initProjectSync } from '@/features/core/dataStore';
import { logger } from '@/utils/appLogger';


export function useAppInitialization(): InitializationState {
    const [state, setState] = useState<InitializationState>({
        status: LoadStatus.Idle,
        error: null,
    });

    // 使用 ref 防止重复初始化项目同步
    const hasRestoredProjectRef = useRef(false);

    const schemaStatus = useSchemaStore((s) => s.status);
    const schemaError = useSchemaStore((s) => s.error);

    const isSchemaReady = schemaStatus === LoadStatus.Ready;

    // 监听依赖状态变化
    useEffect(() => {
        let cancelled = false;

        // 如果已经同步过项目，直接标记完成
        if (hasRestoredProjectRef.current) {
            setState({ status: LoadStatus.Ready, error: null });
            return;
        }

        // 如果已经出错，不继续
        if (state.error) return;

        // 检查是否有错误
        if (schemaError) {
            logger.sys.error('Initialization failed: Schema error ' + schemaError, 'AppInit');
            setState({ status: LoadStatus.Error, error: `Schema: ${schemaError}` });
            return;
        }

        // Schema 加载时会填充 Node Registry，故只需等待 Schema Ready
        if (!isSchemaReady) {
            setState({ status: LoadStatus.Loading, error: null });
            if (schemaStatus === LoadStatus.Idle) {
                useSchemaStore.getState().syncFromBackend();
            }
            return;
        }
        // Schema Ready 后 Registry 已由 schema 填充，无需单独 sync

        // 同步项目状态
        const syncProject = async () => {
            try {
                await initProjectSync();
                if (cancelled) return;
                hasRestoredProjectRef.current = true;
                setState({ status: LoadStatus.Ready, error: null });
            } catch (error) {
                if (cancelled) return;
                const errorMessage = error instanceof Error ? error.message : String(error);
                logger.sys.error('Failed to sync project: ' + errorMessage, 'AppInit');
                setState({
                    status: LoadStatus.Error,
                    error: `Project sync: ${errorMessage}`,
                });
            }
        };

        syncProject();

        return () => {
            cancelled = true;
        };
    }, [isSchemaReady, schemaError, schemaStatus, state.error]);

    return state;
}
