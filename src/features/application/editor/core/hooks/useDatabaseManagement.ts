import { useCallback } from 'react';
import { useProjectStore } from '@/features/core/project';
import { useEditorStore } from '../stores';
import { getUniqueName } from '@/shared/utils';
import { useSidebarTab } from './useSidebarTab';


// database
export function useDatabaseManagement() {
  const switchSidebarTab = useSidebarTab();
  const { selectedItemId, setSelectedInfo } = useEditorStore();

  const addDataFrame = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New DataFrame", Object.values(st.databases));
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
    useProjectStore.getState().updateDatabase(id, data);
  }, []);

  const deleteDataFrame = useCallback((id: string) => {
    useProjectStore.getState().deleteDatabase(id);
    if (selectedItemId === id) setSelectedInfo(null, null);
  }, [selectedItemId, setSelectedInfo]);

  return {
    addDataFrame,
    updateDataFrame,
    deleteDataFrame,
  };
}
