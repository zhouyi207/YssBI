import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/utils";
import { useCustomTitleBar } from "@/features/application/window/useWindowDecorations";
import { WindowTitleBar, WindowTitleBarActions } from "./WindowTitleBar";

export type WindowChromeProps = ComponentProps<"div"> & {
    /** Main editor title bar — higher stacking */
    elevated?: boolean;
    /** Child / satellite window title bar */
    childWindow?: boolean;
    /** Right-side actions; WindowChromeControls should be included when custom chrome is shown */
    actions?: ReactNode;
    children: ReactNode;
};

/**
 * Renders custom title bar chrome when appearance.titleBarStyle is "custom".
 * When "native", returns null — OS frame provides title and window controls.
 */
export function WindowChrome({
    elevated = false,
    childWindow = false,
    actions,
    className,
    children,
    ...props
}: WindowChromeProps) {
    const showCustomChrome = useCustomTitleBar();
    if (!showCustomChrome) return null;

    return (
        <WindowTitleBar elevated={elevated} childWindow={childWindow} className={className} {...props}>
            {children}
            {actions ? <WindowTitleBarActions>{actions}</WindowTitleBarActions> : null}
        </WindowTitleBar>
    );
}

export type WindowMenuBarProps = ComponentProps<"div"> & {
    /** Toolbar buttons shown in both custom and native modes (theme, settings, etc.) */
    toolbar?: ReactNode;
    /** Window chrome controls — only rendered when titleBarStyle is custom */
    windowActions?: ReactNode;
    children: ReactNode;
};

/**
 * Menubar / toolbar row that becomes native-style (no drag region, no window controls)
 * when titleBarStyle is "native".
 */
export function WindowMenuBar({ toolbar, windowActions, className, children, ...props }: WindowMenuBarProps) {
    const showCustomChrome = useCustomTitleBar();

    if (showCustomChrome) {
        return (
            <WindowTitleBar elevated className={cn("menubar-container", className)} {...props}>
                {children}
                <div className="min-w-[20px] flex-1 self-stretch" data-tauri-drag-region />
                {(toolbar || windowActions) ? (
                    <WindowTitleBarActions>
                        {toolbar}
                        {windowActions}
                    </WindowTitleBarActions>
                ) : null}
            </WindowTitleBar>
        );
    }

    return (
        <div
            className={cn(
                "menubar-container flex h-10 shrink-0 items-stretch border-b border-[var(--strong-border)] bg-[var(--panel-header-bg)] shadow-[0_1px_0_rgb(0_0_0/0.08)] select-none",
                className,
            )}
            {...props}
        >
            {children}
            <div className="min-w-0 flex-1" />
            {toolbar ? <div className="flex h-full shrink-0 items-stretch">{toolbar}</div> : null}
        </div>
    );
}
