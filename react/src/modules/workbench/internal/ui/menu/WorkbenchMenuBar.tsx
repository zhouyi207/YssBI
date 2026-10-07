import { Fragment } from "react";

import {
  Menubar as ShadcnMenubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarGroup,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarShortcut,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { BrandLockup } from "@/shared/ui/BrandMark";
import { ToolbarIconButton } from "@/shared/ui/ToolbarIconButton";
import { WindowChromeControls } from "@/shared/ui/WindowChromeControls";
import { WindowMenuBar } from "@/shared/ui/WindowChrome";

export interface WorkbenchMenuItem {
  readonly label: string;
  readonly shortcut?: string;
  readonly onClick?: () => void;
  readonly type?: "item" | "checkbox" | "separator";
  readonly checked?: boolean;
}

export interface WorkbenchMenuDefinition {
  readonly id: string;
  readonly label: string;
  readonly items: readonly WorkbenchMenuItem[];
}

export interface WorkbenchWindowControls {
  readonly maximized: boolean;
  readonly minimize: () => void;
  readonly toggleMaximize: () => void;
  readonly close: () => void;
}

export interface WorkbenchThemeToggle {
  readonly isLightTheme: boolean;
  readonly label: string;
  readonly onToggle: () => void;
}

function selectMenuItem(event: Event, onClick: (() => void) | undefined): void {
  if (!onClick) {
    event.preventDefault();
    return;
  }
  onClick();
}

function MenuButton({ id, label, items }: WorkbenchMenuDefinition) {
  const sections = items.reduce<WorkbenchMenuItem[][]>(
    (groups, item) => {
      if (item.type === "separator" || item.label === "-") {
        if (groups[groups.length - 1]?.length) groups.push([]);
        return groups;
      }

      groups[groups.length - 1]?.push(item);
      return groups;
    },
    [[]],
  );

  return (
    <MenubarMenu value={id}>
      <MenubarTrigger>{label}</MenubarTrigger>
      <MenubarContent>
        {sections.map((section, sectionIndex) => (
          <Fragment key={`${id}-section-${sectionIndex}`}>
            {sectionIndex > 0 ? <MenubarSeparator /> : null}
            <MenubarGroup>
              {section.map((item, itemIndex) => {
                const content = (
                  <>
                    <span className="flex-1">{item.label}</span>
                    {item.shortcut ? <MenubarShortcut>{item.shortcut}</MenubarShortcut> : null}
                  </>
                );

                if (item.type === "checkbox") {
                  return (
                    <MenubarCheckboxItem
                      key={`${id}-${sectionIndex}-${itemIndex}`}
                      checked={item.checked}
                      disabled={!item.onClick}
                      onSelect={(event) => selectMenuItem(event, item.onClick)}
                    >
                      {content}
                    </MenubarCheckboxItem>
                  );
                }

                return (
                  <MenubarItem
                    key={`${id}-${sectionIndex}-${itemIndex}`}
                    disabled={!item.onClick}
                    onSelect={(event) => selectMenuItem(event, item.onClick)}
                  >
                    {content}
                  </MenubarItem>
                );
              })}
            </MenubarGroup>
          </Fragment>
        ))}
      </MenubarContent>
    </MenubarMenu>
  );
}

export function WorkbenchSemanticMenu({
  menus,
}: {
  readonly menus: readonly WorkbenchMenuDefinition[];
}) {
  return (
    <ShadcnMenubar className="border-0">
      {menus.map((menu) => (
        <MenuButton key={menu.id} {...menu} />
      ))}
    </ShadcnMenubar>
  );
}

function ThemeToggleButton({ themeToggle }: { readonly themeToggle: WorkbenchThemeToggle }) {
  return (
    <ToolbarIconButton
      variant="ghost"
      size="icon-lg"
      onClick={themeToggle.onToggle}
      className="self-center text-muted-foreground"
      tooltip={themeToggle.label}
      aria-label={themeToggle.label}
    >
      {themeToggle.isLightTheme ? (
        <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M21 12.8A8.5 8.5 0 1111.2 3a7 7 0 009.8 9.8z"
          />
        </svg>
      ) : (
        <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M12 3v2m0 14v2m9-9h-2M5 12H3m15.36-6.36-1.42 1.42M7.06 16.94l-1.42 1.42m12.72 0-1.42-1.42M7.06 7.06 5.64 5.64"
          />
          <circle cx="12" cy="12" r="4" strokeWidth={2} />
        </svg>
      )}
    </ToolbarIconButton>
  );
}

export function WorkbenchMenuBar({
  menus,
  customChrome,
  themeToggle,
  windowControls,
}: {
  readonly menus: readonly WorkbenchMenuDefinition[];
  readonly customChrome: boolean;
  readonly themeToggle: WorkbenchThemeToggle;
  readonly windowControls: WorkbenchWindowControls;
}) {
  return (
    <WindowMenuBar
      customChrome={customChrome}
      toolbar={<ThemeToggleButton themeToggle={themeToggle} />}
      windowActions={
        <WindowChromeControls
          maximized={windowControls.maximized}
          minimize={windowControls.minimize}
          toggleMaximize={windowControls.toggleMaximize}
          close={windowControls.close}
        />
      }
    >
      <BrandLockup className="pointer-events-none self-center px-4" />
      <WorkbenchSemanticMenu menus={menus} />
    </WindowMenuBar>
  );
}
