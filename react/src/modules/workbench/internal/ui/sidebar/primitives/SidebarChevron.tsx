import { VscChevronRight } from "react-icons/vsc";
import { SIDEBAR_ROW_ICON_SIZE } from "./sidebarStyles";

export function SidebarChevron({ expanded }: { expanded: boolean }) {
  return (
    <span
      className="shrink-0 text-muted-foreground transition-transform duration-150 ease-out"
      style={{ transform: expanded ? "rotate(90deg)" : "rotate(0deg)" }}
    >
      <VscChevronRight size={SIDEBAR_ROW_ICON_SIZE} />
    </span>
  );
}
