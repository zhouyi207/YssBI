import { createContext } from 'react';

/**
 * GroupContext for scoped canvas operations
 * When a component is wrapped in GroupContext.Provider, operations will scope to that group
 */
export const GroupContext = createContext<string | null>(null);
