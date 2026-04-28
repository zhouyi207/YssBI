import { VscDatabase, VscClose } from "react-icons/vsc";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { SqlRemoteTableSelectDialogOptions } from "@/shared/types/ui";
import { OverlayScrollbar } from "./OverlayScrollbar";

const LABELS: Record<string, string> = {
    postgres: "PostgreSQL",
    mysql: "MySQL",
    mariadb: "MariaDB",
};

export const SqlRemoteTableSelectModal = ({
    options,
    onClose,
}: {
    options: SqlRemoteTableSelectDialogOptions;
    onClose: () => void;
}) => {
    const { connectionString, engine, tables, onSelect } = options;
    const label = LABELS[engine] ?? engine;
    const displayName = connectionString.includes("@")
        ? connectionString.replace(/^[^@]+@/, "").replace(/\/.*$/, "")
        : connectionString;

    return (
        <Dialog open onOpenChange={(open) => !open && onClose()}>
            <DialogContent className="max-w-[420px]">
                <DialogHeader className="border-b border-border bg-muted/20">
                    <div className="flex items-center justify-between gap-4">
                        <DialogTitle className="flex items-center gap-2">
                            <VscDatabase className="text-blue-400" size={18} /> 选择表
                        </DialogTitle>
                        <Button type="button" variant="ghost" size="icon-sm" onClick={onClose} aria-label="关闭">
                        <VscClose size={20} />
                        </Button>
                    </div>
                </DialogHeader>

                <div className="p-6">
                    <p className="mb-3 truncate text-xs text-muted-foreground" title={connectionString}>
                        {label} · {displayName}
                    </p>
                    <OverlayScrollbar className="max-h-60">
                        <div className="flex flex-col gap-2">
                            {tables.map((table) => (
                                <Button
                                    key={table}
                                    type="button"
                                    variant="outline"
                                    size="lg"
                                    onClick={() => {
                                        onSelect(table);
                                        onClose();
                                    }}
                                    className="h-auto justify-start gap-3 px-4 py-3 text-left"
                                >
                                    <Badge variant="default">Table</Badge>
                                    <span className="text-sm font-medium text-gray-200">{table}</span>
                                </Button>
                            ))}
                        </div>
                    </OverlayScrollbar>
                </div>

                <DialogFooter className="justify-center">
                    <p className="text-[10px] font-medium text-muted-foreground">选择要导入的表</p>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
};
