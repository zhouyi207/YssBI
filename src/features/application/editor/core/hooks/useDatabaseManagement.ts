import { useCallback } from 'react';
import { useDatabaseStore } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor';
import { getUniqueName } from '@/shared/utils';
import { useSidebarTab } from './useSidebarTab';


// database
export function useDatabaseManagement() {
  const switchSidebarTab = useSidebarTab();
  const selectedItemId = useEditorStore((s) => s.selectedItemId);
  const setSelectedInfo = useEditorStore((s) => s.setSelectedInfo);

  const addDataFrame = useCallback((name?: string) => {
    const st = useDatabaseStore.getState();
    const finalName = getUniqueName(name || "New DataFrame", Object.values(st.databases) as { name: string }[]);
    const id = `df-${crypto.randomUUID()}`;
    const df: any = {
      id,
      name: finalName,
      columns: [],
      rows: [],
      row_count: 0,
      column_count: 0
    };
    st.addDatabase(id, df);
    setSelectedInfo(id, 'data');
    switchSidebarTab('data');
  }, [setSelectedInfo, switchSidebarTab]);

  const updateDataFrame = useCallback((id: string, data: any) => {
    useDatabaseStore.getState().updateDatabase(id, data);
  }, []);

  const deleteDataFrame = useCallback((id: string) => {
    useDatabaseStore.getState().deleteDatabase(id);
    if (selectedItemId === id) setSelectedInfo(null, null);
  }, [selectedItemId, setSelectedInfo]);

  return {
    addDataFrame,
    updateDataFrame,
    deleteDataFrame,
  };
}
