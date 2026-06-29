import React from 'react';
import { UnifiedDataView } from '@/features/core/dataView';
import type { SourceDescriptor } from '@/features/core/dataView';

export type DataViewData = SourceDescriptor;

export interface DataViewComponentProps {
  data: SourceDescriptor;
}

export const DataViewComponent: React.FC<DataViewComponentProps> = ({ data }) => {
  return <UnifiedDataView payload={data} />;
};
