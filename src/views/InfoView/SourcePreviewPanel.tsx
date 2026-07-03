import React from 'react';
import { UnifiedSourceView } from '@/features/core/resultSource';
import type { SourceDescriptor } from '@/features/core/resultSource';

export type SourcePreviewData = SourceDescriptor;

export interface SourcePreviewPanelProps {
  data: SourceDescriptor;
}

export const SourcePreviewPanel: React.FC<SourcePreviewPanelProps> = ({ data }) => {
  return <UnifiedSourceView payload={data} />;
};
