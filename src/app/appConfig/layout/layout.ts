import { LayoutTree } from "@/shared/types";

export const INITIAL_NODES: LayoutTree = {
    ['root']: {
        id: 'root',
        type: 'row',
        parentId: null,
        children: ['sidebar', 'main', 'detail'],
    },
    'sidebar': {
        id: 'sidebar',
        type: 'component',
        parentId: 'root',
        pixelSize: 260, // Default width
        minSize: 240,     // Allow collapsing to 0
        data: { component: 'Sidebar', visible: true, title: 'Explorer', isFixed: true, currentTab: 'events' },
    },
    'main': {
        id: 'main',
        type: 'row',
        parentId: 'root',
        children: ['default_editor'],
        size: 1, // Flex grow
    },
    'default_editor': {
        id: 'default_editor',
        type: 'component',
        parentId: 'main',
        data: {
            component: 'GraphEditor',
            tabs: []
        },
    },
    'detail': {
        id: 'detail',
        type: 'component',
        parentId: 'root',
        pixelSize: 300,
        minSize: 240,
        data: { component: 'Detail', visible: true, title: 'Properties', isFixed: true },
    }
};