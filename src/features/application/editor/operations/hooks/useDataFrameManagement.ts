import { useCallback } from 'react';
import { useProjectStore } from '@/features/core/project';
import { useEditorStore } from '../../core/stores';

const getUniqueName = (baseName: string, items: Record<string, { name: string }>) => {
  const names = Object.values(items).map(i => i.name);
  let name = baseName;
  let counter = 1;
  while (names.includes(name)) {
    name = `${baseName}_${counter}`;
    counter++;
  }
  return name;
};

/**
 * DataFrame Management Hook
 * Handles creation, update, and deletion of dataframes
 */
export function useDataFrameManagement(switchSidebarTab: (tab: 'events' | 'functions' | 'macros' | 'variables' | 'data') => void) {
  const { selectedItemId, setSelectedInfo } = useEditorStore();

  const addDataFrame = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New DataFrame", st.databases);
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
