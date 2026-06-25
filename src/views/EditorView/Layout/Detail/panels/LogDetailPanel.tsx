import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table';
import { LogLevel, LogType } from '@/shared/types/ui';
import { DetailPanelShell } from '../shared/DetailPanelShell';

const LOG_TYPE_LABELS: Record<string, string> = {
  application: 'APP',
  execution: 'EXEC',
  system: 'SYS',
  graph: 'GRAPH',
  data: 'DATA',
};

const getLevelColor = (level: LogLevel) => {
  switch (level) {
    case 'error':
      return 'text-red-400';
    case 'warn':
      return 'text-yellow-400';
    case 'info':
      return 'text-blue-400';
    case 'debug':
      return 'text-gray-400';
    case 'trace':
      return 'text-gray-500';
    default:
      return 'text-gray-400';
  }
};

const getTypeColor = (type: LogType) => {
  switch (type) {
    case 'application':
      return 'text-green-400';
    case 'execution':
      return 'text-purple-400';
    case 'system':
      return 'text-cyan-400';
    case 'graph':
      return 'text-orange-400';
    case 'data':
      return 'text-pink-400';
    default:
      return 'text-gray-400';
  }
};

interface LogDetailPanelProps {
  log: {
    timestamp: string;
    level: LogLevel;
    log_type: LogType;
    source?: string;
    message: string;
  };
}

export function LogDetailPanel({ log }: LogDetailPanelProps) {
  return (
    <DetailPanelShell title="Details : Log">
      <Table className="text-[11px] text-[#cccccc]">
        <TableBody>
          <TableRow>
            <TableCell className="w-20 bg-white/5 font-bold text-gray-400">Time</TableCell>
            <TableCell className="font-mono text-gray-300">{log.timestamp}</TableCell>
          </TableRow>
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400">Level</TableCell>
            <TableCell>
              <span className={`${getLevelColor(log.level)} font-bold uppercase`}>{log.level}</span>
            </TableCell>
          </TableRow>
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400">Type</TableCell>
            <TableCell>
              <span className={`${getTypeColor(log.log_type)} font-semibold`}>
                {LOG_TYPE_LABELS[log.log_type] ?? log.log_type.toUpperCase()}
              </span>
            </TableCell>
          </TableRow>
          {log.source && (
            <TableRow>
              <TableCell className="bg-white/5 font-bold text-gray-400">Source</TableCell>
              <TableCell className="font-mono text-cyan-400">{log.source}</TableCell>
            </TableRow>
          )}
          <TableRow>
            <TableCell className="align-top bg-white/5 font-bold text-gray-400">Message</TableCell>
            <TableCell>
              <pre className="whitespace-pre-wrap break-all font-mono text-[11px] leading-relaxed text-gray-200">
                {log.message}
              </pre>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </DetailPanelShell>
  );
}
