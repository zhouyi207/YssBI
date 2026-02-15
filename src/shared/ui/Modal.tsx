import { DialogOptions } from "@/shared/types/ui/types";

export const Modal = ({ options, onClose }: { options: DialogOptions; onClose: () => void }) => {
  return (
    <div className="fixed inset-0 z-[1000] flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
      <div className="bg-gray-900 border border-gray-700 rounded-lg shadow-2xl w-[400px] overflow-hidden animate-zoom-in">
        <div className="px-6 py-4 border-b border-gray-800 bg-gray-800/50">
          <h3 className="text-lg font-bold text-white">{options.title}</h3>
        </div>
        <div className="px-6 py-6">
          <p className="text-gray-300 text-sm leading-relaxed">{options.message}</p>
        </div>
        <div className="px-6 py-4 bg-gray-800/30 flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-400 hover:text-white hover:bg-gray-700 rounded transition-colors"
          >
            {options.cancelText || "取消"}
          </button>
          <button
            onClick={() => {
              options.onConfirm();
              onClose();
            }}
            className={`px-4 py-2 text-sm text-white rounded transition-all active:scale-95 ${
              options.type === "danger" ? "bg-red-600 hover:bg-red-500" : "bg-blue-600 hover:bg-blue-500"
            }`}
          >
            {options.confirmText || "确定"}
          </button>
        </div>
      </div>
    </div>
  );
};