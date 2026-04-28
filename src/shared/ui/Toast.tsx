import { Message } from "@/shared/types/ui";
import { useEffect } from "react";
import { toast } from "sonner";

export const Toast = ({ message, onClose }: { message: Message; onClose: (id: string) => void }) => {
    useEffect(() => {
        const options = {
            id: message.id,
            duration: message.duration || 3000,
            onDismiss: () => onClose(message.id),
            onAutoClose: () => onClose(message.id),
        };

        switch (message.type) {
            case "success":
                toast.success(message.content, options);
                break;
            case "warning":
                toast.warning(message.content, options);
                break;
            case "error":
                toast.error(message.content, options);
                break;
            case "log":
                toast.message(message.content, options);
                break;
            case "info":
            default:
                toast.info(message.content, options);
                break;
        }
    }, [message, onClose]);

    return null;
};
