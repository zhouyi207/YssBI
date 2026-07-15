import { useEffect, type ComponentProps, type MouseEventHandler } from 'react';
import { cn } from '@/lib/utils';

export type WindowTitleBarProps = ComponentProps<'div'> & {
  /** 子窗口标题栏（Info / Plot / Log 等）提高 stacking */
  childWindow?: boolean;
  /** 主编辑窗口顶层标题栏，提高 z-index */
  elevated?: boolean;
};

/**
 * 统一窗口标题栏壳层，与 Editor Menubar 一致：h-10、workbench 背景、底边与阴影。
 * 右上角不与 CSS 圆角对齐——关闭钮直角贴边，由系统窗口圆角裁剪（与 Edit 页相同）。
 */
export function WindowTitleBar({
  className,
  childWindow = false,
  elevated = false,
  onMouseDownCapture,
  ...props
}: WindowTitleBarProps) {
  useEffect(() => {
    const endDrag = () => window.dispatchEvent(new Event('yssbi-window-drag-end'));
    window.addEventListener('mouseup', endDrag, true);
    window.addEventListener('pointerup', endDrag, true);
    window.addEventListener('blur', endDrag);
    return () => {
      window.removeEventListener('mouseup', endDrag, true);
      window.removeEventListener('pointerup', endDrag, true);
      window.removeEventListener('blur', endDrag);
    };
  }, []);

  const handleMouseDownCapture: MouseEventHandler<HTMLDivElement> = (event) => {
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLElement && activeElement !== event.currentTarget) {
      activeElement.blur();
    }
    window.dispatchEvent(new Event('yssbi-window-drag-start'));
    onMouseDownCapture?.(event);
  };

  return (
    <div
      data-tauri-drag-region
      onMouseDownCapture={handleMouseDownCapture}
      className={cn(
        'flex h-10 shrink-0 items-stretch border-b border-border bg-[var(--workbench-bg)] shadow-xl select-none',
        childWindow && 'z-50',
        elevated ? 'relative z-[100]' : 'relative',
        className,
      )}
      {...props}
    />
  );
}

/** 标题栏右侧工具钮 + 窗口控制区，保证 stretch 与 hover 铺满高度 */
export function WindowTitleBarActions({ className, ...props }: ComponentProps<'div'>) {
  return (
    <div className={cn('flex h-full shrink-0 self-stretch items-stretch', className)} {...props} />
  );
}
