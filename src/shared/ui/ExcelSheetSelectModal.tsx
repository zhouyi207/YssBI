import { VscTable, VscClose } from "react-icons/vsc";
import { ExcelSheetSelectDialogOptions } from "@/shared/types/ui";

export const ExcelSheetSelectModal = ({
  options,
  onClose,
}: {
  options: ExcelSheetSelectDialogOptions;
  onClose: () => void;
}) => {
  const { filePath, sheets, onSelect } = options;
  const fileName = filePath.replace(/^.*[/\\]/, "");

  return (
    <div className="fixed inset-0 z-[500] flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
      <div className="bg-[#1e1e1e] border border-gray-700 rounded-xl shadow-2xl w-[400px] overflow-hidden animate-zoom-in">
        <div className="px-6 py-4 border-b border-gray-800 bg-[#252526] flex justify-between items-center">
          <h3 className="text-sm font-bold text-white flex items-center gap-2 uppercase tracking-wider">
            <VscTable className="text-emerald-500" size={18} /> 选择 Sheet
          </h3>
          <button onClick={onClose} className="text-gray-500 hover:text-white transition-colors">
            <VscClose size={20} />
          </button>
        </div>

        <div className="p-6">
          <p className="text-xs text-gray-500 mb-3 truncate" title={filePath}>
            {fileName}
          </p>
          <div className="flex flex-col gap-2 max-h-60 overflow-y-auto">
            {sheets.map((sheet) => (
              <button
                key={sheet}
                onClick={() => {
                  onSelect(sheet);
                  onClose();
                }}
                className="flex items-center gap-2 px-4 py-3 rounded-lg border border-gray-800 hover:border-[var(--accent-color)] hover:bg-white/5 transition-all text-left"
              >
                <span className="text-[11px] font-mono text-gray-400">SHEET</span>
                <span className="text-sm font-medium text-gray-200">{sheet}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="px-6 py-3 bg-[#252526] border-t border-gray-800 text-center">
          <p className="text-[10px] text-gray-500 font-medium">选择要导入的 Sheet</p>
        </div>
      </div>
    </div>
  );
};
