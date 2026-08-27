// @vitest-environment happy-dom

import { select } from 'd3';
import { afterEach, describe, expect, it } from 'vitest';
import {
  attachMarkTooltip,
  computeAnchorTooltipPosition,
  computePointerTooltipPosition,
  PlotTooltipController,
  tooltipTwoLine,
} from './tooltip';

const CHART_THEME = {
  canvas: '#fff',
  grid: '#eee',
  axis: '#ccc',
  tick: '#999',
  label: '#666',
  zeroLine: '#bbb',
  tooltipBg: '#fff',
  tooltipFg: '#111',
  tooltipMuted: '#888',
};

afterEach(() => document.body.replaceChildren());

describe('chart tooltip positioning', () => {
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

  it('offsets pointer tooltip from the cursor', () => {
    expect(
      computePointerTooltipPosition({
        pointerLeft: 100,
        pointerTop: 50,
      }),
    ).toEqual({ left: 108, top: 14 });
  });
});

describe('tooltipTwoLine', () => {
  it('escapes user-provided text', () => {
    const html = tooltipTwoLine(CHART_THEME, '<lag>', '1 & 2', '#00f');
    expect(html).toContain('&lt;lag&gt;');
    expect(html).toContain('1 &amp; 2');
  });
});

describe('attachMarkTooltip', () => {
  it('gives keyboard focus the same escaped datum tooltip and mark state as pointer hover', () => {
    const container = document.createElement('div');
    const tooltipElement = document.createElement('div');
    const mark = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
    const datum = { label: '<north>', count: 4 };
    container.append(mark, tooltipElement);
    document.body.appendChild(container);

    const tooltip = new PlotTooltipController(tooltipElement, container);
    attachMarkTooltip(select(mark).datum(datum), {
      tooltip,
      getHtml: (value) =>
        tooltipTwoLine(CHART_THEME, value.label, String(value.count), '#00f'),
      getAriaLabel: (value) => `Histogram bin ${value.label}, count ${value.count}`,
      onEnter: (element) => element.setAttribute('data-active', 'true'),
      onLeave: (element) => element.setAttribute('data-active', 'false'),
    });

    expect(mark.getAttribute('tabindex')).toBe('0');
    expect(mark.getAttribute('aria-label')).toBe('Histogram bin <north>, count 4');

    mark.dispatchEvent(new MouseEvent('mouseenter', { clientX: 20, clientY: 20 }));
    const pointerHtml = tooltipElement.innerHTML;
    expect(pointerHtml).toContain('&lt;north&gt;');
    expect(tooltipElement.style.opacity).toBe('1');
    expect(mark.getAttribute('data-active')).toBe('true');
    mark.dispatchEvent(new MouseEvent('mouseleave'));
    expect(tooltipElement.style.opacity).toBe('0');
    expect(mark.getAttribute('data-active')).toBe('false');

    mark.dispatchEvent(new FocusEvent('focus'));
    expect(tooltipElement.innerHTML).toBe(pointerHtml);
    expect(tooltipElement.style.opacity).toBe('1');
    expect(mark.getAttribute('data-active')).toBe('true');
    mark.dispatchEvent(new FocusEvent('blur'));
    expect(tooltipElement.style.opacity).toBe('0');
    expect(mark.getAttribute('data-active')).toBe('false');
  });
});
