import { Message } from "@/shared/types/ui";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import React from "react";

export const Toast = ({ message, onClose }: { message: Message; onClose: (id: string) => void }) => {
    const tone = {
        info: "default",
        success: "success",
        warning: "warning",
        error: "destructive",
        log: "outline",
    }[message.type];

    React.useEffect(() => {
        const timer = setTimeout(() => onClose(message.id), message.duration || 3000);
        return () => clearTimeout(timer);
    }, [message, onClose]);

    return (
        <Card className="flex max-w-[360px] items-center gap-3 border-border/80 bg-card/95 px-4 py-3 shadow-2xl backdrop-blur animate-slide-in">
            <Badge variant={tone}>{message.type}</Badge>
            <span className="min-w-0 flex-1 text-sm font-medium text-card-foreground">{message.content}</span>
            <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={() => onClose(message.id)}
                className="text-muted-foreground hover:text-foreground"
                aria-label="关闭提示"
            >
                x
            </Button>
        </Card>
    );
};
