import type { Presentation } from './types';

export function presentationRoute(presentation: Presentation): string {
  switch (presentation.kind) {
    case 'inspector':
      return '/view';
    case 'plot':
      return '/plot';
    case 'report':
      return '/info';
  }
}

export function plotTypeFromPresentation(presentation: Presentation): string | undefined {
  return presentation.kind === 'plot' ? presentation.chart : undefined;
}

export function presentationRouteForDescriptor(descriptor: {
  presentation: Presentation;
}): string {
  return presentationRoute(descriptor.presentation);
}
