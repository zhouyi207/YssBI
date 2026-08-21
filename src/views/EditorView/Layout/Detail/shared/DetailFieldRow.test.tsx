import { describe, expect, it } from 'vitest';
import { DetailReadonlyField } from './DetailForm';
import { DetailFieldRow } from './DetailFieldRow';

describe('DetailFieldRow', () => {
  it('uses shared 40/60 property columns with truncated labels and a full-width value cell', () => {
    const element = DetailFieldRow({
      label: '名称',
      children: 'A very long value',
    });
    const [label, value] = element.props.children as Array<{
      props: { className?: string; title?: string };
    }>;

    expect(element.props.className).toContain('grid-cols-[minmax(0,2fr)_minmax(0,3fr)]');
    expect(label.props.className).toContain('truncate');
    expect(label.props.title).toBe('名称');
    expect(value.props.className).toContain('w-full');
    expect(value.props.className).toContain('text-right');
  });

  it('renders readonly values right-aligned and truncated to one line', () => {
    const field = DetailReadonlyField({ label: '名称', children: 'A very long value' });
    const value = field.props.children;

    expect(value.props.className).toContain('justify-end');
    expect(value.props.className).toContain('truncate');
    expect(value.props.className).toContain('text-right');
  });
});
