import React, { useState, useRef, useEffect } from "react";
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
            <div
                onClick={() => !disabled && setIsOpen(!isOpen)}
                className={`flex items-center justify-between px-2 py-1 bg-[#3c3c3c] border ${isOpen ? "border-[#007acc]" : "border-transparent"} hover:border-[#007acc] rounded transition-all`}
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
            </div>

            {isOpen && (
                <div className="absolute z-50 w-full mt-0.5 bg-[#252526] border border-[#454545] shadow-2xl rounded overflow-hidden">
                    <OverlayScrollbar className="max-h-60 py-1" direction="vertical">
                        {formattedOptions.map((option) => (
                            <div
                                key={option.value}
                                onClick={() => {
                                    onChange(option.value);
                                    setIsOpen(false);
                                }}
                                className={`px-2 py-1.5 hover:bg-[#007acc] hover:text-white transition-colors cursor-pointer ${value === option.value ? "bg-[#37373d] text-white" : "text-[#cccccc]"
                                    }`}
                            >
                                {option.label}
                            </div>
                        ))}
                    </OverlayScrollbar>
                </div>
            )}
        </div>
    );
};
