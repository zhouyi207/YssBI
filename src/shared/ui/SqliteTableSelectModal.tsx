import { useTranslation } from "react-i18next";
import { VscDatabase, VscClose } from "react-icons/vsc";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { SqliteTableSelectDialogOptions } from "@/shared/types/ui";
import { OverlayScrollbar } from "./OverlayScrollbar";

export const SqliteTableSelectModal = ({
  options,
  onClose,
}: {
  options: SqliteTableSelectDialogOptions;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const { dbPath, tables, onSelect } = options;
  const dbName = dbPath.replace(/^.*[/\\]/, "");

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-[420px]">
        <DialogHeader className="border-b border-border bg-muted/20">
          <div className="flex items-center justify-between gap-4">
            <DialogTitle className="flex items-center gap-2">
              <VscDatabase className="text-blue-400" size={18} /> {t("importModal.selectTable")}
            </DialogTitle>
            <Button type="button" variant="ghost" size="icon-sm" onClick={onClose} aria-label={t("importModal.close")}>
            <VscClose size={20} />
            </Button>
          </div>
        </DialogHeader>

        <div className="p-6">
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="mb-3 truncate text-xs text-muted-foreground">{dbName}</p>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="max-w-sm break-all">{dbPath}</TooltipContent>
          </Tooltip>
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
                  <span className="text-sm font-medium text-foreground">{table}</span>
                </Button>
              ))}
            </div>
          </OverlayScrollbar>
        </div>

        <DialogFooter className="justify-center">
          <p className="text-[10px] font-medium text-muted-foreground">{t("importModal.tableHint")}</p>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
