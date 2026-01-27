/**
 * 前端日志工具 - 将日志转发到后端
 * 使用 tauri-plugin-log 实现统一日志输出
 */

import { info, debug, warn, error, trace } from '@tauri-apps/plugin-log';

// 重新导出 tauri log 函数供直接使用
export { info, debug, warn, error, trace };

/**
 * 应用级日志器
 * 提供类似 console 的 API，但日志会转发到后端
 */
export const logger = {
    /**
     * 信息日志
     */
    info: (message: string, ...args: unknown[]) => {
        const formattedMessage = formatMessage(message, args);
        info(formattedMessage);
        // 同时在浏览器控制台输出（开发时方便）
        if (import.meta.env.DEV) {
            console.log(`[INFO] ${formattedMessage}`);
        }
    },

    /**
     * 调试日志
     */
    debug: (message: string, ...args: unknown[]) => {
        const formattedMessage = formatMessage(message, args);
        debug(formattedMessage);
        if (import.meta.env.DEV) {
            console.log(`[DEBUG] ${formattedMessage}`);
        }
    },

    /**
     * 警告日志
     */
    warn: (message: string, ...args: unknown[]) => {
        const formattedMessage = formatMessage(message, args);
        warn(formattedMessage);
        if (import.meta.env.DEV) {
            console.warn(`[WARN] ${formattedMessage}`);
        }
    },

    /**
     * 错误日志
     */
    error: (message: string, ...args: unknown[]) => {
        const formattedMessage = formatMessage(message, args);
        error(formattedMessage);
        if (import.meta.env.DEV) {
            console.error(`[ERROR] ${formattedMessage}`);
        }
    },

    /**
     * 追踪日志（最详细）
     */
    trace: (message: string, ...args: unknown[]) => {
        const formattedMessage = formatMessage(message, args);
        trace(formattedMessage);
        if (import.meta.env.DEV) {
            console.log(`[TRACE] ${formattedMessage}`);
        }
    },
};

/**
 * 格式化日志消息，支持对象参数
 */
function formatMessage(message: string, args: unknown[]): string {
    if (args.length === 0) {
        return message;
    }

    // 将参数转换为字符串
    const argsStr = args.map(arg => {
        if (typeof arg === 'object') {
            try {
                return JSON.stringify(arg, null, 2);
            } catch {
                return String(arg);
            }
        }
        return String(arg);
    }).join(' ');

    return `${message} ${argsStr}`;
}

/**
 * 安装全局 console 拦截（可选）
 * 调用此函数后，所有 console.log 等调用都会被转发到后端
 */
export function installGlobalConsoleForwarder() {
    const originalConsole = {
        log: console.log,
        info: console.info,
        warn: console.warn,
        error: console.error,
        debug: console.debug,
    };

    console.log = (...args: unknown[]) => {
        originalConsole.log(...args);
        const message = args.map(arg => 
            typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' ');
        info(message).catch(() => {});
    };

    console.info = (...args: unknown[]) => {
        originalConsole.info(...args);
        const message = args.map(arg => 
            typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' ');
        info(message).catch(() => {});
    };

    console.warn = (...args: unknown[]) => {
        originalConsole.warn(...args);
        const message = args.map(arg => 
            typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' ');
        warn(message).catch(() => {});
    };

    console.error = (...args: unknown[]) => {
        originalConsole.error(...args);
        const message = args.map(arg => 
            typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' ');
        error(message).catch(() => {});
    };

    console.debug = (...args: unknown[]) => {
        originalConsole.debug(...args);
        const message = args.map(arg => 
            typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' ');
        debug(message).catch(() => {});
    };

    info('[Logger] Global console forwarder installed').catch(() => {});
}

export default logger;
