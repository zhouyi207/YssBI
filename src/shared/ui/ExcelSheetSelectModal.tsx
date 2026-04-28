import { VscTable, VscClose } from "react-icons/vsc";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ExcelSheetSelectDialogOptions } from "@/shared/types/ui";
import { OverlayScrollbar } from "./OverlayScrollbar";

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
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-[420px]">
        <DialogHeader className="border-b border-border bg-muted/20">
          <div className="flex items-center justify-between gap-4">
            <DialogTitle className="flex items-center gap-2">
              <VscTable className="text-emerald-400" size={18} /> 选择 Sheet
            </DialogTitle>
            <Button type="button" variant="ghost" size="icon-sm" onClick={onClose} aria-label="关闭">
            <VscClose size={20} />
            </Button>
          </div>
        </DialogHeader>

        <div className="p-6">
          <p className="mb-3 truncate text-xs text-muted-foreground" title={filePath}>
            {fileName}
          </p>
          <OverlayScrollbar className="max-h-60">
            <div className="flex flex-col gap-2">
              {sheets.map((sheet) => (
                <Button
                  key={sheet}
                  type="button"
                  variant="outline"
                  size="lg"
                  onClick={() => {
                    onSelect(sheet);
                    onClose();
                  }}
                  className="h-auto justify-start gap-3 px-4 py-3 text-left"
                >
                  <Badge variant="success">Sheet</Badge>
                  <span className="text-sm font-medium text-gray-200">{sheet}</span>
                </Button>
              ))}
            </div>
          </OverlayScrollbar>
        </div>

        <DialogFooter className="justify-center">
          <p className="text-[10px] font-medium text-muted-foreground">选择要导入的 Sheet</p>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
