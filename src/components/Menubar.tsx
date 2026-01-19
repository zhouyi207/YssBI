import { useCanvas } from "./canvas/CanvasContext";

export default function Menubar() {
  const { exportGraph, importGraph, executeGraph } = useCanvas();

  return (
    <div 
      className="menubar-container h-12 bg-gray-900 border-b border-gray-800 flex items-center px-6 gap-6 z-50 shadow-xl"
      onWheel={(e) => e.stopPropagation()}
    >
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 bg-blue-600 rounded-lg flex items-center justify-center shadow-lg shadow-blue-900/20">
          <span className="text-white font-black text-lg">Y</span>
        </div>
        <div className="text-white font-bold text-base tracking-tight">
          Yss<span className="text-blue-500">BI</span>
        </div>
      </div>
      
      <div className="h-6 w-px bg-gray-700 mx-2" />

      <div className="flex items-center gap-2">
        <button 
          onClick={() => importGraph()}
          className="group flex items-center gap-2 px-3 py-1.5 rounded-md text-gray-400 hover:text-white hover:bg-gray-800 transition-all duration-200 active:scale-95"
          title="Import Graph (Ctrl+O)"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
          </svg>
          <span className="text-sm font-medium">导入</span>
        </button>
        <button 
          onClick={() => exportGraph()}
          className="group flex items-center gap-2 px-3 py-1.5 rounded-md text-gray-400 hover:text-white hover:bg-gray-800 transition-all duration-200 active:scale-95"
          title="Export Graph (Ctrl+S)"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
          </svg>
          <span className="text-sm font-medium">导出</span>
        </button>
      </div>

      <div className="h-6 w-px bg-gray-700 mx-2" />

      <div className="flex items-center">
        <button
          onClick={() => executeGraph()}
          className="flex items-center gap-2 px-4 py-1.5 rounded-md bg-green-600 hover:bg-green-500 text-white transition-all duration-200 active:scale-95 shadow-lg shadow-green-900/20"
        >
          <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          <span className="text-sm font-bold">执行</span>
        </button>
      </div>

      <div className="flex-1" />
      
      <div className="text-[10px] text-gray-500 font-mono">
        v1.0.0
      </div>
    </div>
  );
}
