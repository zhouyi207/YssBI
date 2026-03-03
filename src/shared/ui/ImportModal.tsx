import { useState } from "react";
import { VscDatabase, VscClose, VscFile, VscTable, VscCloudDownload } from "react-icons/vsc";
import { BsDatabaseFill } from "react-icons/bs";
import { ImportDialogOptions, ImportDataSourceType } from "@/shared/types/ui";

type ImportTypeConfig = {
  id: ImportDataSourceType;
  label: string;
  icon: React.ReactNode;
  color: string;
  comingSoon: boolean;
};

type CategoryId = "file" | "sql" | "other";

const CATEGORIES: { id: CategoryId; label: string; icon: React.ReactNode; color: string }[] = [
  { id: "file", label: "文件", icon: <VscFile className="text-green-500" size={20} />, color: "bg-green-500/20" },
  { id: "sql", label: "SQL 数据库", icon: <BsDatabaseFill className="text-blue-500" size={20} />, color: "bg-blue-500/20" },
  { id: "other", label: "其他", icon: <VscCloudDownload className="text-purple-500" size={20} />, color: "bg-purple-500/20" },
];

const FILE_TYPES: ImportTypeConfig[] = [
  { id: "csv", label: "CSV", icon: <VscFile className="text-green-500" size={24} />, color: "bg-green-500/10", comingSoon: false },
  { id: "xlsx", label: "Excel", icon: <VscTable className="text-emerald-500" size={24} />, color: "bg-emerald-500/10", comingSoon: false },
];

const SQL_TYPES: ImportTypeConfig[] = [
  { id: "sqlite", label: "SQLite", icon: <BsDatabaseFill className="text-blue-500" size={24} />, color: "bg-blue-500/10", comingSoon: false },
  { id: "postgres", label: "PostgreSQL", icon: <BsDatabaseFill className="text-cyan-500" size={24} />, color: "bg-cyan-500/10", comingSoon: false },
  { id: "mysql", label: "MySQL", icon: <BsDatabaseFill className="text-orange-500" size={24} />, color: "bg-orange-500/10", comingSoon: false },
  { id: "mariadb", label: "MariaDB", icon: <BsDatabaseFill className="text-amber-500" size={24} />, color: "bg-amber-500/10", comingSoon: false },
];

const OTHER_TYPES: ImportTypeConfig[] = [
  { id: "api", label: "REST API", icon: <VscCloudDownload className="text-purple-500" size={24} />, color: "bg-purple-500/10", comingSoon: true },
];

const CATEGORY_TYPES: Record<CategoryId, ImportTypeConfig[]> = {
  file: FILE_TYPES,
  sql: SQL_TYPES,
  other: OTHER_TYPES,
};

function TypeOptionButton({
  type,
  onSelect,
  onClose,
}: {
  type: ImportTypeConfig;
  onSelect: (id: ImportDataSourceType) => void;
  onClose: () => void;
}) {
  return (
    <button
      onClick={() => {
        onSelect(type.id);
        if (!type.comingSoon) onClose();
      }}
      className={`group flex flex-col items-center gap-1.5 p-3 rounded-lg border transition-all active:scale-[0.98] ${
        type.comingSoon
          ? "border-gray-800/50 bg-gray-800/20 opacity-60 cursor-default"
          : "border-gray-800 hover:border-[var(--accent-color)] hover:bg-white/5"
      }`}
      title={type.comingSoon ? "功能开发中" : undefined}
    >
      <div
        className={`flex items-center justify-center w-12 h-12 rounded-full ${type.color} transition-transform ${
          !type.comingSoon && "group-hover:scale-110"
        }`}
      >
        {type.icon}
      </div>
      <span className={`text-[11px] font-bold uppercase tracking-tight ${type.comingSoon ? "text-gray-500" : "text-gray-300 group-hover:text-white"}`}>
        {type.label}
        {type.comingSoon && <span className="block text-[9px] font-normal text-gray-600 mt-0.5">开发中</span>}
      </span>
    </button>
  );
}

export const ImportModal = ({ options, onClose }: { options: ImportDialogOptions; onClose: () => void }) => {
  const [selectedCategory, setSelectedCategory] = useState<CategoryId>("file");
  const types = CATEGORY_TYPES[selectedCategory];

  return (
    <div className="fixed inset-0 z-[500] flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
      <div className="bg-[#1e1e1e] border border-gray-700 rounded-xl shadow-2xl w-[420px] h-[320px] overflow-hidden animate-zoom-in flex flex-col">
        {/* 标题栏：横跨整个弹窗顶部 */}
        <div className="px-6 py-4 border-b border-gray-800 bg-[#252526] flex justify-between items-center flex-shrink-0">
          <h3 className="text-sm font-bold text-white flex items-center gap-2 uppercase tracking-wider">
            <VscDatabase className="text-blue-500" size={18} /> 导入外部数据
          </h3>
          <button onClick={onClose} className="text-gray-500 hover:text-white transition-colors">
            <VscClose size={20} />
          </button>
        </div>

        {/* 内容区：左侧分类 + 右侧图标 */}
        <div className="flex flex-1 min-h-0">
          <div className="w-[120px] border-r border-gray-800 bg-[#252526] flex flex-col py-2 flex-shrink-0">
            {CATEGORIES.map((cat) => (
              <button
                key={cat.id}
                onClick={() => setSelectedCategory(cat.id)}
                className={`flex items-center gap-2 px-3 py-2.5 text-left transition-colors ${
                  selectedCategory === cat.id
                    ? "bg-white/10 border-l-2 border-[var(--accent-color)] text-white"
                    : "text-gray-400 hover:bg-white/5 hover:text-gray-300"
                }`}
              >
                <div className={`w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 ${cat.color}`}>
                  {cat.icon}
                </div>
                <span className="text-[11px] font-medium truncate">{cat.label}</span>
              </button>
            ))}
          </div>

          <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
            <div className="p-4 overflow-y-auto h-full">
              <div className="grid grid-cols-2 gap-3">
                {types.map((type) => (
                  <TypeOptionButton key={type.id} type={type} onSelect={options.onSelect} onClose={onClose} />
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
