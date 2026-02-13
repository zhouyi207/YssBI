import { useCallback } from 'react';
import { DataFrameData } from '@/shared/types/editor';
import { useProjectStore } from '@/features/project';
import { useEditorStore } from '../stores';

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
    const finalName = getUniqueName(name || "New DataFrame", st.dataframes);
    const id = `df-${crypto.randomUUID()}`;
    const df: DataFrameData = {
      id,
      name: finalName,
      columns: [],
      rows: [],
      rowCount: 0,
      columnCount: 0
    };
    st.addDataFrame(id, df);
    setSelectedInfo(id, 'data');
    switchSidebarTab('data');
  }, [setSelectedInfo, switchSidebarTab]);

  const updateDataFrame = useCallback((id: string, data: Partial<DataFrameData>) => {
    useProjectStore.getState().updateDataFrame(id, data);
  }, []);

  const deleteDataFrame = useCallback((id: string) => {
    useProjectStore.getState().deleteDataFrame(id);
    if (selectedItemId === id) setSelectedInfo(null, null);
  }, [selectedItemId, setSelectedInfo]);

  return {
    addDataFrame,
    updateDataFrame,
    deleteDataFrame,
  };
}
