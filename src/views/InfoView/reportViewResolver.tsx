import type { ComponentType } from 'react';
import type { ReportKind } from '@/features/application/viewCapabilities';
import { OLSComponent } from './OLSComponent';
import { VARComponent } from './VARComponent';
import { VARSocComponent } from './VARSocComponent';
import { VECComponent } from './VECComponent';
import { VecRankComponent } from './VecRankComponent';
import { DFADFComponent } from './DFADFComponent';
import { DFADFSummaryListComponent } from './DFADFSummaryListComponent';
import { BinaryComponent } from './BinaryComponent';
import { PanelComponent } from './PanelComponent';
import { DIDComponent } from './DIDComponent';
import { PraisComponent } from './PraisComponent';
import { TwoSLSComponent } from './2SLSComponent';
import { LIMLComponent } from './LIMLComponent';

export type ReportViewProps = { data: unknown };

const REPORT_COMPONENTS: Record<ReportKind, ComponentType<ReportViewProps>> = {
  olsSummary: OLSComponent as ComponentType<ReportViewProps>,
  binarySummary: BinaryComponent as ComponentType<ReportViewProps>,
  iv2slsSummary: TwoSLSComponent as ComponentType<ReportViewProps>,
  ivLimlSummary: LIMLComponent as ComponentType<ReportViewProps>,
  praisSummary: PraisComponent as ComponentType<ReportViewProps>,
  varSummary: VARComponent as ComponentType<ReportViewProps>,
  varSoc: VARSocComponent as ComponentType<ReportViewProps>,
  panelSummary: PanelComponent as ComponentType<ReportViewProps>,
  panelDid: DIDComponent as ComponentType<ReportViewProps>,
  dfAdfSummary: DFADFComponent as ComponentType<ReportViewProps>,
  dfAdfSummaryList: DFADFSummaryListComponent as ComponentType<ReportViewProps>,
  vecSummary: VECComponent as ComponentType<ReportViewProps>,
  vecRankSummary: VecRankComponent as ComponentType<ReportViewProps>,
};

export function resolveReportComponent(report: ReportKind): ComponentType<ReportViewProps> {
  return REPORT_COMPONENTS[report] ?? OLSComponent;
}
