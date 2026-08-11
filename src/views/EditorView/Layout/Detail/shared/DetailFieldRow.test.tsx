import { describe, expect, it } from 'vitest';
import { DetailReadonlyField } from './DetailForm';
import { DetailFieldRow } from './DetailFieldRow';

describe('DetailFieldRow', () => {
  it('uses content-width labels by default', () => {
    const element = DetailFieldRow({
      label: '名称',
      children: 'A very long value',
    });

    expect(element.props.className).toContain('grid-cols-[max-content_minmax(0,1fr)]');
  });

  it('renders readonly values left-aligned and truncated to one line', () => {
    const field = DetailReadonlyField({ label: '名称', children: 'A very long value' });
    const value = field.props.children;

    expect(value.props.className).toContain('justify-start');
    expect(value.props.className).toContain('truncate');
    expect(value.props.className).toContain('text-left');
  });
});
