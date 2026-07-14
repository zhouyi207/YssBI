import { useState, useEffect } from 'react';
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

    const schemaStatus = useSchemaStore((s) => s.status);
    const schemaError = useSchemaStore((s) => s.error);

    const isSchemaReady = schemaStatus === LoadStatus.Ready;

    useEffect(() => {
        let cancelled = false;

        if (state.error) return;

        if (schemaError) {
            logger.sys.error('Initialization failed: Schema error ' + schemaError, 'AppInit');
            setState({ status: LoadStatus.Error, error: `Schema: ${schemaError}` });
            return;
        }

        if (!isSchemaReady) {
            setState({ status: LoadStatus.Loading, error: null });
            if (schemaStatus === LoadStatus.Idle) {
                useSchemaStore.getState().syncFromBackend();
            }
            return;
        }

        const syncProject = async () => {
            try {
                await initProjectSync();
                if (cancelled) return;
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
