import React, { createContext, useContext, useState, useCallback, ReactNode } from "react";

// --- Types ---

export type MessageType = "info" | "success" | "warning" | "error" | "log";

export interface Message {
  id: string;
  type: MessageType;
  content: string;
  duration?: number;
}

export interface DialogOptions {
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  type?: "danger" | "info";
  onConfirm: () => void;
}

interface UIContextValue {
  showToast: (content: string, type?: MessageType, duration?: number) => void;
  showDialog: (options: DialogOptions) => void;
}

const UIContext = createContext<UIContextValue | null>(null);

// --- Components ---

const Toast = ({ message, onClose }: { message: Message; onClose: (id: string) => void }) => {
  const bgColor = {
    info: "bg-blue-600",
    success: "bg-green-600",
    warning: "bg-yellow-600",
    error: "bg-red-600",
    log: "bg-gray-800 border border-gray-700",
  }[message.type];

  React.useEffect(() => {
    const timer = setTimeout(() => onClose(message.id), message.duration || 3000);
    return () => clearTimeout(timer);
  }, [message, onClose]);

  return (
    <div className={`${bgColor} text-white px-4 py-2 rounded shadow-lg flex items-center gap-3 animate-slide-in`}>
      <span className="text-sm font-medium">{message.content}</span>
      <button onClick={() => onClose(message.id)} className="opacity-50 hover:opacity-100">×</button>
    </div>
  );
};

const Modal = ({ options, onClose }: { options: DialogOptions; onClose: () => void }) => {
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

// --- Provider ---

export const UIProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [messages, setMessages] = useState<Message[]>([]);
  const [dialog, setDialog] = useState<DialogOptions | null>(null);

  const showToast = useCallback((content: string, type: MessageType = "info", duration = 3000) => {
    const id = Math.random().toString(36).substring(7);
    setMessages(prev => [...prev, { id, type, content, duration }]);
  }, []);

  const closeToast = useCallback((id: string) => {
    setMessages(prev => prev.filter(m => m.id !== id));
  }, []);

  const showDialog = useCallback((options: DialogOptions) => {
    setDialog(options);
  }, []);

  return (
    <UIContext.Provider value={{ showToast, showDialog }}>
      {children}
      
      {/* Toast Container */}
      <div className="fixed bottom-6 right-6 z-[2000] flex flex-col gap-3">
        {messages.map(m => (
          <Toast key={m.id} message={m} onClose={closeToast} />
        ))}
      </div>

      {/* Dialog */}
      {dialog && <Modal options={dialog} onClose={() => setDialog(null)} />}
    </UIContext.Provider>
  );
};

export const useUI = () => {
  const ctx = useContext(UIContext);
  if (!ctx) throw new Error("useUI must be used within UIProvider");
  return ctx;
};
