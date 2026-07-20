import { describe, expect, it } from 'vitest';
import { selectParameterForTask } from './useBayesPlotData';

describe('Bayes plot parameter selection', () => {
  it('keeps a valid selection for the same task', () => {
    expect(selectParameterForTask('task-1', ['a', 'b'], { taskId: 'task-1', parameter: 'b' })).toBe('b');
  });

  it('resets to the first parameter for a new result', () => {
    expect(selectParameterForTask('task-2', ['a', 'b'], { taskId: 'task-1', parameter: 'b' })).toBe('a');
  });

  it('falls back when the selected parameter disappears', () => {
    expect(selectParameterForTask('task-1', ['a'], { taskId: 'task-1', parameter: 'b' })).toBe('a');
  });
});
