import { Message } from "@/shared/types/ui";
import React from "react";

export const Toast = ({ message, onClose }: { message: Message; onClose: (id: string) => void }) => {
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
            <button onClick={() => onClose(message.id)} className="opacity-50 hover:opacity-100">x</button>
        </div>
    );
};
