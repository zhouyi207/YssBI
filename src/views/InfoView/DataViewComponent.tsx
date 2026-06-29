import React from 'react';
import { UnifiedDataView } from '@/features/core/dataView';
import type { DataViewPayload } from '@/features/core/dataView';

export type DataViewData = DataViewPayload;

export interface DataViewComponentProps {
  data: DataViewPayload;
}

export const DataViewComponent: React.FC<DataViewComponentProps> = ({ data }) => {
  return <UnifiedDataView payload={data} />;
};
