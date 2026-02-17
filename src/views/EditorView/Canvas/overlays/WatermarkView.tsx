import { useEditorGroup } from "@/features/application/editor";

export const WatermarkView = () => {
  const { addEvent, addFunction, addMacro, importGraph } = useEditorGroup();

  return (
    <div className="relative w-full h-full flex flex-col items-center justify-center bg-[var(--workbench-bg)] select-none overflow-hidden">
      {/* Simplified Logo */}
      <div className="mb-8 opacity-20 group">
        <svg className="w-32 h-32 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1">
          <path strokeLinecap="round" strokeLinejoin="round" d="M11 3.055A9.001 9.001 0 1020.945 13H11V3.055z" />
          <path strokeLinecap="round" strokeLinejoin="round" d="M20.488 9H15V3.512A9.025 9.025 0 0120.488 9z" />
        </svg>
      </div>
      {/* Shortcut Hints */}
      <div className="flex flex-col gap-4 items-start text-gray-500 text-sm font-medium">
        <div className="flex items-center gap-12 justify-between w-full min-w-[340px] hover:bg-white/5 p-2 rounded transition-colors group cursor-pointer" onClick={() => addEvent()}>
          <div className="flex items-center gap-2">
            <div className="w-1.5 h-1.5 rounded-full bg-red-500" />
            <span>新建 Event Graph</span>
          </div>
          <span className="text-[10px] text-gray-600 italic">Core logic</span>
        </div>
        <div className="flex items-center gap-12 justify-between w-full hover:bg-white/5 p-2 rounded transition-colors group cursor-pointer" onClick={() => addFunction()}>
          <div className="flex items-center gap-2">
            <div className="w-1.5 h-1.5 rounded-full bg-blue-500" />
            <span>新建 Function</span>
          </div>
          <span className="text-[10px] text-gray-600 italic">Reusable routine</span>
        </div>
        <div className="flex items-center gap-12 justify-between w-full hover:bg-white/5 p-2 rounded transition-colors group cursor-pointer" onClick={() => addMacro()}>
          <div className="flex items-center gap-2">
            <div className="w-1.5 h-1.5 rounded-full bg-purple-500" />
            <span>新建 Macro</span>
          </div>
          <span className="text-[10px] text-gray-600 italic">Node pattern</span>
        </div>
        <div className="flex items-center gap-12 justify-between w-full hover:bg-white/5 p-2 rounded transition-colors group cursor-pointer" onClick={() => importGraph()}>
          <span>打开文件</span>
          <span className="flex gap-1">
            <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">Ctrl</kbd>
            <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">O</kbd>
          </span>
        </div>
        <div className="flex items-center gap-12 justify-between w-full hover:bg-white/5 p-2 rounded transition-colors group cursor-not-allowed opacity-50">
          <span>显示所有命令</span>
          <span className="flex gap-1">
            <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">Ctrl</kbd>
            <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">Shift</kbd>
            <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">P</kbd>
          </span>
        </div>
      </div>
      {/* Subtle grid background for the empty state too, but very faint */}
      <div className="absolute inset-0 opacity-[0.03] pointer-events-none"
        style={{
          backgroundImage: `linear-gradient(#fff 1px, transparent 1px), linear-gradient(90deg, #fff 1px, transparent 1px)`,
          backgroundSize: '40px 40px'
        }}
      />
    </div>
  );
};
