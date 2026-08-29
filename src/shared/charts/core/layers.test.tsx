// @vitest-environment happy-dom

import { select } from 'd3';
import { describe, expect, it } from 'vitest';
import { joinCartesianLayers } from './layers';

const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';

describe('joinCartesianLayers', () => {
  it('reuses exactly one of each named cartesian layer', () => {
    const svg = document.createElementNS(SVG_NAMESPACE, 'svg');
    const title = document.createElementNS(SVG_NAMESPACE, 'title');
    svg.append(title);
    const selection = select(svg);

    joinCartesianLayers(selection);
    const firstRoot = svg.querySelector('[data-chart-layer="root"]');
    joinCartesianLayers(selection);

    expect(svg.querySelector('title')).toBe(title);
    expect(svg.querySelectorAll('[data-chart-layer="root"]')).toHaveLength(1);
    expect(svg.querySelector('[data-chart-layer="root"]')).toBe(firstRoot);

    for (const name of ['grid', 'x-axis', 'y-axis', 'marks', 'labels']) {
      const layers = svg.querySelectorAll(`[data-chart-layer="${name}"]`);
      expect(layers).toHaveLength(1);
      expect(layers[0]?.parentElement).toBe(firstRoot);
    }
  });
});
