import { useTranslation } from "react-i18next";
import { useStatusBarItems } from "@/features/application/statusBar/useStatusBarItems";
import { StatusBarItem } from "./StatusBarItem";

export function BottomBar() {
  const { t } = useTranslation();
  const { left, right } = useStatusBarItems();

  return (
    <footer
      className="flex h-(--statusbar-height) shrink-0 items-center justify-between overflow-hidden border-t border-(--strong-border) bg-(--panel-header-bg) text-[11px] font-medium text-foreground"
      aria-label={t("bottomBar.ariaLabel")}
    >
      <div className="flex h-full min-w-0 items-center">
        {left.map((item) => (
          <StatusBarItem key={item.id} item={item} />
        ))}
      </div>
      <div className="flex h-full shrink-0 items-center">
        {right.map((item) => (
          <StatusBarItem key={item.id} item={item} />
        ))}
      </div>
    </footer>
  );
}
