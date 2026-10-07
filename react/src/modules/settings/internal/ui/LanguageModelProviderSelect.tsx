import { useTranslation } from "react-i18next";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@/components/ui/combobox";
import type { LanguageModelCatalog } from "@/services/assistant/modelContract";

type ProviderOption = Pick<LanguageModelCatalog["presets"][number], "id" | "name">;

export function LanguageModelProviderSelect({
  id,
  value,
  name,
  presets,
  disabled,
  onSelect,
}: {
  id: string;
  value: string;
  name: string;
  presets: LanguageModelCatalog["presets"];
  disabled: boolean;
  onSelect: (preset: LanguageModelCatalog["presets"][number]) => void;
}) {
  const { t } = useTranslation();
  const selected =
    presets.find((preset) => preset.id === value) ?? (name ? { id: value, name } : null);

  return (
    <Combobox<ProviderOption>
      items={presets}
      value={selected}
      itemToStringLabel={(preset) => preset.name}
      itemToStringValue={(preset) => preset.id}
      isItemEqualToValue={(preset, current) => preset.id === current.id}
      onValueChange={(choice) => {
        const preset = presets.find((entry) => entry.id === choice?.id);
        if (preset) onSelect(preset);
      }}
      disabled={disabled}
      autoHighlight
      required
    >
      <ComboboxInput
        id={id}
        aria-label={t("settings.models.providerName")}
        className="w-full"
        disabled={disabled}
        placeholder={t("settings.models.searchProviders")}
      />
      <ComboboxContent>
        <ComboboxEmpty>{t("settings.models.noMatchingProviders")}</ComboboxEmpty>
        <ComboboxList>
          {(preset: ProviderOption) => (
            <ComboboxItem key={preset.id} value={preset}>
              <span className="truncate">{preset.name}</span>
            </ComboboxItem>
          )}
        </ComboboxList>
      </ComboboxContent>
    </Combobox>
  );
}
