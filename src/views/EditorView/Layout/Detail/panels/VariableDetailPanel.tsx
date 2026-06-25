import { Input } from '@/components/ui/input';
import { Select } from '@/shared/ui';
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table';
import { dataTypeKind, dataTypeFromKey, isPrimitiveType } from '@/shared/types/domain/dataType';
import { dataValueToRaw, dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';

interface VariableDetailPanelProps {
  variable: {
    id: string;
    name: string;
    dataType: import('@/shared/types/domain/dataType').DataType;
    dataValue: import('@/shared/types/domain/dataValue').DataValue;
  };
  onUpdate: (patch: Record<string, unknown>) => void;
  onDelete: () => Promise<void>;
  onDeleted: () => void;
}

export function VariableDetailPanel({
  variable,
  onUpdate,
  onDelete,
  onDeleted,
}: VariableDetailPanelProps) {
  return (
    <DetailPanelShell title={`Details : ${variable.name}`}>
      <Table className="text-[11px] text-[#cccccc]">
        <TableBody>
          <TableRow>
            <TableCell className="w-20 bg-white/5 font-bold text-gray-400">Name</TableCell>
            <TableCell>
              <Input
                className="h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none"
                value={variable.name}
                onChange={(e) => onUpdate({ name: e.target.value })}
              />
            </TableCell>
          </TableRow>
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400">Type</TableCell>
            <TableCell>
              <Select
                value={dataTypeKind(variable.dataType)}
                options={[
                  { label: 'Boolean', value: 'Boolean' },
                  { label: 'Int32', value: 'Int32' },
                  { label: 'Int64', value: 'Int64' },
                  { label: 'Float32', value: 'Float32' },
                  { label: 'Float64', value: 'Float64' },
                  { label: 'String', value: 'String' },
                  { label: 'Object', value: 'Object' },
                  { label: 'Any', value: 'Any' },
                  { label: 'DataFrame', value: 'DataFrame' },
                  { label: 'Array', value: 'Array' },
                ]}
                onChange={(val) => onUpdate({ dataType: dataTypeFromKey(val as string) })}
              />
            </TableCell>
          </TableRow>
          {variable.dataType.kind !== 'Array' && isPrimitiveType(variable.dataType) && (
            <TableRow>
              <TableCell className="bg-white/5 font-bold text-gray-400">Value</TableCell>
              <TableCell>
                {variable.dataType.kind === 'Boolean' ? (
                  <Input
                    type="checkbox"
                    className="h-4 w-4 accent-[var(--accent-color)]"
                    checked={!!dataValueToRaw(variable.dataValue)}
                    onChange={(e) =>
                      onUpdate({ dataValue: dataValueFromRaw(e.target.checked, variable.dataType) })
                    }
                  />
                ) : (
                  <Input
                    className="h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none"
                    type={variable.dataType.kind === 'String' ? 'text' : 'number'}
                    value={String(dataValueToRaw(variable.dataValue) ?? '')}
                    onChange={(e) => {
                      const val =
                        variable.dataType.kind === 'String'
                          ? e.target.value
                          : Number(e.target.value);
                      onUpdate({ dataValue: dataValueFromRaw(val, variable.dataType) });
                    }}
                  />
                )}
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
      <DetailDeleteButton
        itemType="variable"
        itemName={variable.name}
        onDelete={onDelete}
        onDeleted={onDeleted}
      />
    </DetailPanelShell>
  );
}
