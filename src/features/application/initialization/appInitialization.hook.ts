import { useState, useEffect } from 'react';
import { InitializationState } from './appInitialization.type';
import { LoadStatus } from '@/shared/types/ui';
import { initProjectSync } from '@/features/core/dataStore';
import { logger } from '@/utils/appLogger';

export function useAppInitialization(): InitializationState {
    const [state, setState] = useState<InitializationState>({
        status: LoadStatus.Idle,
        error: null,
    });

    useEffect(() => {
        let cancelled = false;

        setState({ status: LoadStatus.Loading, error: null });

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

        void syncProject();

        return () => {
            cancelled = true;
        };
    }, []);

    return state;
}
