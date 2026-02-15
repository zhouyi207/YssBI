import { VscDatabase, VscClose, VscFile, VscTable, VscCloudDownload } from "react-icons/vsc";
import { BsDatabaseFill } from "react-icons/bs";
import { ImportDialogOptions } from "@/features/core/ui/types";

export const ImportModal = ({ options, onClose }: { options: ImportDialogOptions; onClose: () => void }) => {
  const types = [
    { id: 'csv', label: 'CSV 文件', icon: <VscFile className="text-green-500" />, color: 'bg-green-500/10' },
    { id: 'xlsx', label: 'Excel (XLSX)', icon: <VscTable className="text-emerald-500" />, color: 'bg-emerald-500/10' },
    { id: 'sql', label: 'SQL 数据库', icon: <BsDatabaseFill className="text-blue-500" />, color: 'bg-blue-500/10' },
    { id: 'api', label: 'REST API', icon: <VscCloudDownload className="text-purple-500" />, color: 'bg-purple-500/10' }
  ] as const;

  return (
    <div className="fixed inset-0 z-[1000] flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
      <div className="bg-[#1e1e1e] border border-gray-700 rounded-xl shadow-2xl w-[420px] overflow-hidden animate-zoom-in">
        <div className="px-6 py-4 border-b border-gray-800 bg-[#252526] flex justify-between items-center">
          <h3 className="text-sm font-bold text-white flex items-center gap-2 uppercase tracking-wider">
            <VscDatabase className="text-blue-500" size={18} /> 导入外部数据
          </h3>
          <button onClick={onClose} className="text-gray-500 hover:text-white transition-colors">
            <VscClose size={20} />
          </button>
        </div>

        <div className="p-6 grid grid-cols-2 gap-4">
          {types.map(type => (
            <button
              key={type.id}
              onClick={() => {
                options.onSelect(type.id);
                onClose();
              }}
              className="flex flex-col items-center gap-3 p-5 rounded-xl border border-gray-800 hover:border-[var(--accent-color)] hover:bg-white/5 transition-all group active:scale-95"
            >
              <div className={`w-12 h-12 rounded-full flex items-center justify-center text-2xl ${type.color} group-hover:scale-110 transition-transform`}>
                {type.icon}
              </div>
              <span className="text-[11px] font-bold text-gray-300 group-hover:text-white uppercase tracking-tight">{type.label}</span>
            </button>
          ))}
        </div>

        <div className="px-6 py-3 bg-[#252526] border-t border-gray-800 text-center">
          <p className="text-[10px] text-gray-500 font-medium">选择数据源类型开始导入流程</p>
        </div>
      </div>
    </div>
  );
};
