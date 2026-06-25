import React from 'react';
import { GraphEditor } from "../Canvas/core/GraphEditor";
import { SettingsView } from "../Layout/SettingsView";
import Sidebar from "../Layout/Sidebar";
import { Detail } from "../Layout/Detail/Detail";
import { LogPanel } from "@/views/LogView/LogPanel";

/**
 * 视图注册表类
 * 用于管理字符串 ID 与 React 组件之间的映射
 */
type ComponentType = React.ComponentType<any>;

class ViewRegistry {
    private static instance: ViewRegistry;
    private components: Map<string, ComponentType> = new Map();

    private constructor() { }

    public static getInstance(): ViewRegistry {
        if (!ViewRegistry.instance) {
            ViewRegistry.instance = new ViewRegistry();
        }
        return ViewRegistry.instance;
    }

    public register(id: string, component: ComponentType) {
        this.components.set(id, component);
    }

    public get(id: string): ComponentType | undefined {
        return this.components.get(id);
    }
}

export const viewRegistry = ViewRegistry.getInstance();

// --- 注册编辑器核心业务组件 ---

// 1. 蓝图图形编辑器
viewRegistry.register('GraphEditor', GraphEditor);

// 2. 设置编辑器
viewRegistry.register('SettingsEditor', SettingsView);

// 3. 侧边栏 (Explorer)
viewRegistry.register('Sidebar', Sidebar);

// 4. 属性详情栏 (Properties)
viewRegistry.register('Detail', Detail);

// 5. 日志面板 (Logs - VSCode-style panel at bottom)
viewRegistry.register('LogPanel', LogPanel);
