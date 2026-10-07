import React from "react";
import {
  Select as ShadcnSelect,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface Option {
  label: string;
  value: string;
}

interface SelectProps {
  options: (string | Option)[];
  value: string;
  onChange: (value: string) => void;
  className?: string;
  disabled?: boolean;
  id?: string;
  onOpenChange?: (open: boolean) => void;
  statusText?: string;
}

export const Select: React.FC<SelectProps> = ({
  options,
  value,
  onChange,
  className = "",
  disabled = false,
  id,
  onOpenChange,
  statusText,
}) => {
  const formattedOptions: Option[] = options.map((opt) =>
    typeof opt === "string" ? { label: opt, value: opt } : opt,
  );
  const hasEmptyOption = formattedOptions.some((option) => option.value === "");
  // Radix reserves the empty string for its placeholder; prefix every option to avoid collisions.
  const selectValue = value !== "" || hasEmptyOption ? `:${value}` : "";

  return (
    <ShadcnSelect
      value={selectValue}
      onValueChange={(nextValue) => onChange(nextValue.slice(1))}
      disabled={disabled}
      onOpenChange={onOpenChange}
    >
      <SelectTrigger id={id} size="sm" className={className}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {statusText && (
          <div role="status" className="px-2 py-1 text-xs text-muted-foreground">
            {statusText}
          </div>
        )}
        {formattedOptions.map((option) => (
          <SelectItem key={option.value} value={`:${option.value}`}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </ShadcnSelect>
  );
};
