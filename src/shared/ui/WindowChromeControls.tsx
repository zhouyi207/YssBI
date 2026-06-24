import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { cn } from '@/lib/utils';

function MinimizeIcon() {
  return (
    <svg className="size-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden>
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
    </svg>
  );
}

function MaximizeIcon() {
  return (
    <svg className="size-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden>
      <rect x="4" y="4" width="16" height="16" strokeWidth={2} />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden>
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
    </svg>
  );
}

const chromeButtonBase = cn(
  'flex shrink-0 self-stretch items-center justify-center rounded-none border-0 p-0 leading-none',
  'text-muted-foreground outline-none select-none transition-colors',
  'hover:bg-muted hover:text-foreground dark:hover:bg-muted/50',
  'focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/30',
);

export interface WindowChromeControlsProps {
  isMaximized?: boolean;
  onMinimize?: () => void | Promise<void>;
  onMaximize?: () => void | Promise<void>;
  onClose?: () => void | Promise<void>;
  /** 一般省略：直角贴边，由系统窗口圆角裁剪。仅当外层壳层显式设了匹配圆角时再传入 */
  closeCornerClassName?: string;
  className?: string;
}

/**
 * 窗口标题栏右侧：最小化 / 最大化 / 关闭。
 * 按钮 hover 背景铺满标题栏高度；关闭钮默认直角贴边，与 Edit 主窗口一致。
 */
export function WindowChromeControls({
  isMaximized = false,
  onMinimize,
  onMaximize,
  onClose,
  closeCornerClassName,
  className,
}: WindowChromeControlsProps) {
  const { t } = useTranslation();

  const handleMinimize = () => {
    if (onMinimize) void onMinimize();
    else void getCurrentWindow().minimize();
  };

  const handleMaximize = () => {
    if (onMaximize) void onMaximize();
    else void getCurrentWindow().toggleMaximize();
  };

  const handleClose = () => {
    if (onClose) void onClose();
    else void getCurrentWindow().close();
  };

  return (
    <div className={cn('flex self-stretch', className)}>
      <button
        type="button"
        onClick={handleMinimize}
        className={cn(chromeButtonBase, 'w-10')}
        title={t('common.minimize')}
        aria-label={t('common.minimize')}
      >
        <MinimizeIcon />
      </button>
      <button
        type="button"
        onClick={handleMaximize}
        className={cn(chromeButtonBase, 'w-10')}
        title={isMaximized ? t('common.restore') : t('common.maximize')}
        aria-label={isMaximized ? t('common.restore') : t('common.maximize')}
      >
        <MaximizeIcon />
      </button>
      <button
        type="button"
        onClick={handleClose}
        className={cn(
          chromeButtonBase,
          'w-11 hover:bg-red-600 hover:text-white dark:hover:bg-red-600',
          closeCornerClassName,
        )}
        title={t('common.close')}
        aria-label={t('common.close')}
      >
        <CloseIcon />
      </button>
    </div>
  );
}
