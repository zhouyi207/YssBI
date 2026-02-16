import React from 'react';
import { Sidebar } from '@/views/EditorView/Layout/Sidebar';
import { Detail } from '@/views/EditorView/Layout/Detail';
import { settingsEditor } from "@/views/EditorView/Layout/SettingsEditor";
import { GraphEditor } from '@/views/EditorView/Layout/GraphEditor';

/**
 * 视图注册表类
 * 用于管理字符串 ID 与 React 组件之间的映射
 */
type ComponentType = React.ComponentType<any>;

class LayoutViewRegistry {
    private static instance: LayoutViewRegistry;
    private components: Map<string, ComponentType> = new Map();

    private constructor() { }

    public static getInstance(): LayoutViewRegistry {
        if (!LayoutViewRegistry.instance) {
            LayoutViewRegistry.instance = new LayoutViewRegistry();
        }
        return LayoutViewRegistry.instance;
    }

    public register(id: string, component: ComponentType) {
        this.components.set(id, component);
    }

    public get(id: string): ComponentType | undefined {
        return this.components.get(id);
    }
}

export const viewRegistry = LayoutViewRegistry.getInstance();

// --- 注册编辑器核心业务组件 ---

// 1. 蓝图图形编辑器
viewRegistry.register('GraphEditor', GraphEditor);

// 2. 设置编辑器
viewRegistry.register('SettingsEditor', settingsEditor);

// 3. 侧边栏 (Explorer)
viewRegistry.register('Sidebar', Sidebar);

// 4. 属性详情栏 (Properties)
viewRegistry.register('Detail', Detail);
