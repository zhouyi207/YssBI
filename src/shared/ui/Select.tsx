import React, { useState, useRef, useEffect } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { OverlayScrollbar } from "./OverlayScrollbar";

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
}

export const Select: React.FC<SelectProps> = ({ options, value, onChange, className = "", disabled = false }) => {
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);

    const formattedOptions: Option[] = options.map(opt =>
        typeof opt === "string" ? { label: opt, value: opt } : opt
    );

    const selectedOption = formattedOptions.find(opt => opt.value === value) || formattedOptions[0];

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        document.addEventListener("mousedown", handleClickOutside);
        return () => document.removeEventListener("mousedown", handleClickOutside);
    }, []);

    return (
        <div
            ref={containerRef}
            className={`relative inline-block w-full text-xs select-none ${className} ${disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
        >
            <Button
                type="button"
                onClick={() => !disabled && setIsOpen(!isOpen)}
                variant="outline"
                size="sm"
                disabled={disabled}
                className={`w-full justify-between ${isOpen ? "border-[var(--accent-color)]" : ""}`}
            >
                <span className="truncate text-[#cccccc]">{selectedOption?.label}</span>
                <svg
                    className={`w-3 h-3 text-[#858585] transition-transform ${isOpen ? "rotate-180" : ""}`}
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                >
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
            </Button>

            {isOpen && (
                <Card className="absolute z-50 mt-1 w-full overflow-hidden shadow-2xl">
                    <OverlayScrollbar className="max-h-60 py-1" direction="vertical">
                        {formattedOptions.map((option) => (
                            <Button
                                type="button"
                                variant="ghost"
                                key={option.value}
                                onClick={() => {
                                    onChange(option.value);
                                    setIsOpen(false);
                                }}
                                className={`h-auto w-full justify-between rounded-none px-3 py-2 text-left ${value === option.value ? "bg-muted text-white" : "text-[#cccccc]"}`}
                            >
                                <span className="truncate">{option.label}</span>
                                {value === option.value && <Badge variant="outline">当前</Badge>}
                            </Button>
                        ))}
                    </OverlayScrollbar>
                </Card>
            )}
        </div>
    );
};
