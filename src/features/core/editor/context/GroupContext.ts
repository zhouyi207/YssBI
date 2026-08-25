import { createContext } from 'react';

/** Group scope for layout-backed consumers such as the worksheet editor. */
export const GroupContext = createContext<string | null>(null);
