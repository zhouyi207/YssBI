import { Message } from "@/shared/types/ui";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { useEffect, useRef } from "react";
import { toast } from "sonner";

export const Toast = ({ message, onClose }: { message: Message; onClose: (id: string) => void }) => {
    const onCloseRef = useRef(onClose);
    onCloseRef.current = onClose;
    const content = formatErrorMessage(message.content, "");

    useEffect(() => {
        const options = {
            id: message.id,
            duration: message.duration || 3000,
            onDismiss: () => onCloseRef.current(message.id),
            onAutoClose: () => onCloseRef.current(message.id),
        };

        switch (message.type) {
            case "success":
                toast.success(content, options);
                break;
            case "warning":
                toast.warning(content, options);
                break;
            case "error":
                toast.error(content, options);
                break;
            case "log":
                toast.message(content, options);
                break;
            case "info":
            default:
                toast.info(content, options);
                break;
        }
    }, [message.id, message.type, content, message.duration]);

    return null;
};
