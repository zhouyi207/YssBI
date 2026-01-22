import React, { createContext, useContext, useState, useEffect, ReactNode } from "react";

interface ThemeSettings {
    workbenchBackground: string;
    sidebarBackground: string;
    accentColor: string;
    gridLines: string;
    nodeBase: string;
    connectionLines: string;
    selectionRegion: string;
    // Pin & Type Colors
    execColor: string;
    intColor: string;
    floatColor: string;
    boolColor: string;
    stringColor: string;
    objectColor: string;
}

interface ThemeContextValue {
    theme: ThemeSettings;
    updateTheme: (updates: Partial<ThemeSettings>) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

const DEFAULT_THEME: ThemeSettings = {
    workbenchBackground: "#181818",
    sidebarBackground: "#1f1f1f",
    accentColor: "#007acc",
    gridLines: "#222222",
    nodeBase: "#2b2b2b",
    connectionLines: "#888888",
    selectionRegion: "#007acc33",
    execColor: "#ffffff",
    intColor: "#3592c4",
    floatColor: "#4ab3e3",
    boolColor: "#c94f4f",
    stringColor: "#7bb0a6",
    objectColor: "#9179c9",
};

export const ThemeProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [theme, setTheme] = useState<ThemeSettings>(DEFAULT_THEME);

    const updateTheme = (updates: Partial<ThemeSettings>) => {
        setTheme(prev => ({ ...prev, ...updates }));
    };

    useEffect(() => {
        const root = document.documentElement;
        root.style.setProperty("--workbench-bg", theme.workbenchBackground);
        root.style.setProperty("--sidebar-bg", theme.sidebarBackground);
        root.style.setProperty("--accent-color", theme.accentColor);
        root.style.setProperty("--grid-lines", theme.gridLines);
        root.style.setProperty("--node-base", theme.nodeBase);
        root.style.setProperty("--connection-lines", theme.connectionLines);
        root.style.setProperty("--selection-region", theme.selectionRegion);

        root.style.setProperty("--exec-color", theme.execColor);
        root.style.setProperty("--int-color", theme.intColor);
        root.style.setProperty("--float-color", theme.floatColor);
        root.style.setProperty("--bool-color", theme.boolColor);
        root.style.setProperty("--string-color", theme.stringColor);
        root.style.setProperty("--object-color", theme.objectColor);

        // Compute some derived colors if needed (like hover states)
        root.style.setProperty("--accent-color-hover", theme.accentColor + "cc"); // simplified transparent hover
    }, [theme]);

    return (
        <ThemeContext.Provider value={{ theme, updateTheme }}>
            {children}
        </ThemeContext.Provider>
    );
};

export const useTheme = () => {
    const ctx = useContext(ThemeContext);
    if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
    return ctx;
};
