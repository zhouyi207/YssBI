import React from 'react';
import { VscRefresh, VscDiscard, VscExport } from 'react-icons/vsc';
import type { EditState } from '@/features/core/dataStore/editStateStore';
import { Select } from '@/shared/ui';

interface DataframeOption { label: string; value: string; }

interface ToolbarProps {
  selectedDfId: string | null;
  options: DataframeOption[];
  loading: boolean;
  totalRowCount: number;
  columnCount: number;
  hasSelection: boolean;
  currentEditState: EditState;
  onSelectDf: (id: string) => void;
  onRefresh: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onReset: () => void;
  onExport: () => void;
}

export const Toolbar: React.FC<ToolbarProps> = ({
  selectedDfId, options, loading, totalRowCount, columnCount, hasSelection,
  currentEditState, onSelectDf, onRefresh, onUndo, onRedo, onReset, onExport,
}) => (
  <div className="h-12 border-b border-gray-800 flex items-center px-4 gap-2 bg-[var(--sidebar-bg)] shrink-0">
    <div className="w-[240px]">
      <Select
        value={selectedDfId || ''}
        onChange={onSelectDf}
        options={options}
        disabled={options.length === 0 || (options.length === 1 && options[0]?.value === '')}
      />
    </div>

    <button onClick={onRefresh} className="p-1.5 hover:bg-white/5 rounded transition-colors text-gray-400 hover:text-white" title="Refresh">
      <VscRefresh className={loading ? 'animate-spin' : ''} size={15} />
    </button>

    <div className="w-px h-5 bg-gray-700 mx-1" />

    <button onClick={onUndo} disabled={!currentEditState.canUndo} className="p-1.5 hover:bg-white/5 rounded transition-colors disabled:opacity-20 text-gray-400 hover:text-white" title="Undo (Ctrl+Z)">
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 10h13a4 4 0 010 8H9" /><path d="M3 10l4-4M3 10l4 4" /></svg>
    </button>
    <button onClick={onRedo} disabled={!currentEditState.canRedo} className="p-1.5 hover:bg-white/5 rounded transition-colors disabled:opacity-20 text-gray-400 hover:text-white" title="Redo (Ctrl+Shift+Z)">
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 10H8a4 4 0 000 8h6" /><path d="M21 10l-4-4M21 10l-4 4" /></svg>
    </button>

    <button onClick={onReset} disabled={!currentEditState.isModified} className="p-1.5 hover:bg-white/5 rounded transition-colors disabled:opacity-20 text-gray-400 hover:text-white" title="Reset to Original">
      <VscDiscard size={15} />
    </button>

    <div className="w-px h-5 bg-gray-700 mx-1" />

    <button onClick={onExport} disabled={!hasSelection} className="p-1.5 hover:bg-white/5 rounded transition-colors disabled:opacity-20 text-gray-400 hover:text-white" title="Export">
      <VscExport size={15} />
    </button>

    {hasSelection && (
      <div className="ml-auto flex items-center gap-4 text-[10px] font-mono opacity-50">
        <span>COLUMNS: {columnCount}</span>
        <span>ROWS: {totalRowCount}</span>
      </div>
    )}
  </div>
);
