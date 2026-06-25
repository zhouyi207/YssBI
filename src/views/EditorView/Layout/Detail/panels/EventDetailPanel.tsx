import { Input } from '@/components/ui/input';
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';

interface EventDetailPanelProps {
  event: { id: string; name: string };
  onUpdate: (patch: Record<string, unknown>) => void;
  onDelete: () => Promise<void>;
  onDeleted: () => void;
}

export function EventDetailPanel({ event, onUpdate, onDelete, onDeleted }: EventDetailPanelProps) {
  return (
    <DetailPanelShell title={`Details : ${event.name}`}>
      <Table className="text-[11px] text-[#cccccc]">
        <TableBody>
          <TableRow>
            <TableCell className="w-20 bg-white/5 font-bold text-gray-400">Name</TableCell>
            <TableCell>
              <Input
                className="h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none"
                value={event.name}
                onChange={(e) => onUpdate({ name: e.target.value })}
              />
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
      <DetailDeleteButton
        itemType="event"
        itemName={event.name}
        onDelete={onDelete}
        onDeleted={onDeleted}
      />
    </DetailPanelShell>
  );
}
