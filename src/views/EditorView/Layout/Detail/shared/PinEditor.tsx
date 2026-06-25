import { Select } from '@/shared/ui';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

interface PinEditorProps {
  title: string;
  pins: Array<{ id: string; name: string; type: string; containerType?: string }>;
  onChange: (pins: PinEditorProps['pins']) => void;
}

export function PinEditor({ title, pins, onChange }: PinEditorProps) {
  return (
    <div className="mt-4 px-2">
      <div className="mb-1 flex items-center justify-between">
        <span className="text-[10px] font-black uppercase text-gray-400">{title}</span>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={() => {
            onChange([...pins, { id: `pin-${crypto.randomUUID()}`, name: 'NewPin', type: 'int' }]);
          }}
          className="text-muted-foreground hover:text-[var(--accent-color)]"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </Button>
      </div>
      <div className="space-y-1">
        {pins.map((pin, idx) => (
          <div key={pin.id} className="group flex items-center gap-1 rounded bg-white/5 p-1">
            <Input
              className="h-6 flex-1 border-0 bg-transparent px-1 py-0 text-[10px] shadow-none"
              value={pin.name}
              onChange={(e) => {
                const newPins = [...pins];
                newPins[idx] = { ...newPins[idx], name: e.target.value };
                onChange(newPins);
              }}
            />
            <Select
              className="w-24"
              value={pin.type}
              options={['exec', 'int', 'float', 'bool', 'string', 'object']}
              onChange={(val) => {
                const newPins = [...pins];
                newPins[idx] = { ...newPins[idx], type: val };
                onChange(newPins);
              }}
            />
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={() => {
                const newPins = [...pins];
                const current = newPins[idx].containerType;
                const next = current === 'array' ? 'dataseries' : current === 'dataseries' ? undefined : 'array';
                newPins[idx] = { ...newPins[idx], containerType: next };
                onChange(newPins);
              }}
              className={pin.containerType ? 'bg-blue-500/10 text-blue-400' : 'text-muted-foreground'}
              title={`Container: ${pin.containerType ?? 'none'} (click to cycle)`}
            >
              <span className="text-[9px] font-black">
                {pin.containerType === 'dataseries' ? '◇' : pin.containerType === 'array' ? '[]' : '·'}
              </span>
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={() => {
                onChange(pins.filter((_, i) => i !== idx));
              }}
              className="opacity-0 transition-opacity group-hover:opacity-100 hover:text-red-500"
            >
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </Button>
          </div>
        ))}
        {pins.length === 0 && (
          <div className="py-1 text-center text-[9px] italic text-gray-300">
            No {title.toLowerCase()}
          </div>
        )}
      </div>
    </div>
  );
}
