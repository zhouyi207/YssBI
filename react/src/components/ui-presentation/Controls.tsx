import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

export function InfoSegmentedToggle<T extends string>({
  value,
  onValueChange,
  options,
}: {
  value: T;
  onValueChange: (value: T) => void;
  options: { value: T; label: string }[];
}) {
  return (
    <ToggleGroup
      type="single"
      value={value}
      onValueChange={(next) => next && onValueChange(next as T)}
      variant="outline"
      size="sm"
      className="text-[11px]"
    >
      {options.map((option) => (
        <ToggleGroupItem key={option.value} value={option.value} className="px-3">
          {option.label}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}
