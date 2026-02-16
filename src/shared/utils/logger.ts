import { warn, debug, info, error } from '@tauri-apps/plugin-log';

function forwardConsole(
    fnName: 'log' | 'debug' | 'info' | 'warn' | 'error',
    logger: (message: string) => Promise<void>
) {
    const original = console[fnName];
    console[fnName] = (...args: any[]) => {
        // 格式化所有参数
        const message = args.map(arg => {
            if (typeof arg === 'string') {
                return arg;
            }
            try {
                return JSON.stringify(arg, null, 2);
            } catch {
                return String(arg);
            }
        }).join(' ');

        // 同时输出到原始控制台和 Tauri 日志
        original(...args);
        logger(message);
    };
}

export const setupLogger = () => {
    forwardConsole('log', info);
    forwardConsole('debug', debug);
    forwardConsole('info', info);
    forwardConsole('warn', warn);
    forwardConsole('error', error);
};

