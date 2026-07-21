import { KDEChart, type KDEChartProps, type KDEPoint } from '@/shared/charts';
import { plotContainerClass } from './plotShellStyles';

export type { KDEPoint };
export type KDEProps = Omit<KDEChartProps, 'className'>;

/** PlotView compatibility wrapper; shared rendering lives in shared/charts. */
export default function KDE(props: KDEProps) {
  return <KDEChart {...props} className={plotContainerClass(undefined, props.height)} />;
}
