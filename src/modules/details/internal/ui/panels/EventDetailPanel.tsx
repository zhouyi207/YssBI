import { useTranslation } from "react-i18next";
import { DetailPanelShell } from "../shared/DetailPanelShell";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";
import { GraphConstantsPanel } from "./GraphConstantsPanel";

interface EventDetailPanelProps {
  graphPath: string;
  event: { name: string };
}

export function EventDetailPanel({ event, graphPath }: EventDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t("detail.fields.name")} tone="body">
          {event.name}
        </DetailReadonlyField>
      </DetailForm>
      <GraphConstantsPanel key={graphPath} graphPath={graphPath} />
    </DetailPanelShell>
  );
}
