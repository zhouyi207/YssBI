import { Message } from "@/shared/types/ui";
import { useEffect, useRef } from "react";
import { toast } from "sonner";

export const Toast = ({ message, onClose }: { message: Message; onClose: (id: string) => void }) => {
    const onCloseRef = useRef(onClose);
    onCloseRef.current = onClose;

    useEffect(() => {
        const options = {
            id: message.id,
            duration: message.duration || 3000,
            onDismiss: () => onCloseRef.current(message.id),
            onAutoClose: () => onCloseRef.current(message.id),
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
    }, [message.id, message.type, message.content, message.duration]);

    return null;
};
