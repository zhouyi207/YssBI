import { describe, expect, it } from 'vitest';
import {
  computeAnchorTooltipPosition,
  computePointerTooltipPosition,
  tooltipTwoLine,
} from './d3Tooltip';

describe('d3Tooltip positioning', () => {
  it('anchors tooltip above when there is room', () => {
    expect(
      computeAnchorTooltipPosition({
        containerWidth: 200,
        anchorLeft: 80,
        anchorTop: 40,
        anchorWidth: 20,
        anchorHeight: 10,
        tooltipWidth: 60,
        tooltipHeight: 24,
      }),
    ).toEqual({ left: 60, top: 10 });
  });

  it('flips anchored tooltip below when above is clipped', () => {
    expect(
      computeAnchorTooltipPosition({
        containerWidth: 200,
        anchorLeft: 80,
        anchorTop: 8,
        anchorWidth: 20,
        anchorHeight: 10,
        tooltipWidth: 60,
        tooltipHeight: 24,
        padding: 6,
      }),
    ).toEqual({ left: 60, top: 24 });
  });

  it('centers pointer tooltip when requested', () => {
    expect(
      computePointerTooltipPosition({
        containerWidth: 200,
        containerHeight: 120,
        pointerLeft: 100,
        pointerTop: 50,
        tooltipWidth: 40,
        tooltipHeight: 20,
        centerX: true,
      }),
    ).toEqual({ left: 80, top: 22 });
  });
});

describe('tooltipTwoLine', () => {
  it('escapes user-provided text', () => {
    const html = tooltipTwoLine(
      {
        canvas: '#fff',
        grid: '#eee',
        axis: '#ccc',
        tick: '#999',
        label: '#666',
        zeroLine: '#bbb',
        tooltipBg: '#fff',
        tooltipFg: '#111',
        tooltipMuted: '#888',
      },
      '<lag>',
      '1 & 2',
      '#00f',
    );
    expect(html).toContain('&lt;lag&gt;');
    expect(html).toContain('1 &amp; 2');
  });
});
