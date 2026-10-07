import { useTranslation } from "react-i18next";

import { useStatusBarItems } from "@/features/application/statusBar/useStatusBarItems";
import { StatusBar } from "@/modules/workbench/public";

export function WorkbenchStatusBarContribution() {
  const { t } = useTranslation();
  const { left, right } = useStatusBarItems();
  return <StatusBar ariaLabel={t("bottomBar.ariaLabel")} left={left} right={right} />;
}
