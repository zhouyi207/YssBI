import { describe, expect, it } from 'vitest';
import { createEventHandlers } from '../handlers';
import { isValidEventType } from '../utils/eventParser';
import { EventRegistry } from './EventRegistry';


describe('EventRegistry project mutation registrations', () => {
  it('registers revisioned project mutation events', () => {
    const registered = new EventRegistry(createEventHandlers()).getRegisteredTypes();

    expect(registered).toEqual(expect.arrayContaining([
      'GraphDelta',
      'ResourceMutationCommitted',
      'ComputationSettingsChanged',
    ]));
  });

  it('keeps active project wires and rejects producerless legacy wires', () => {
    const registered = new EventRegistry(createEventHandlers()).getRegisteredTypes();
    const removedTypes = [
      'EventUpdated',
      'EventDeleted',
      'FunctionUpdated',
      'FunctionDeleted',
      'ResourceChanged',
      'DataFrameCreated',
      'DataFrameDeleted',
      'VariableCreated',
      'VariableUpdated',
      'VariableDeleted',
    ];

    expect(registered).toEqual(expect.arrayContaining([
      'ProjectLoaded',
      'ProjectCleared',
      'ProjectLifecycleCommitted',
      'ProjectSaved',
      'ProjectIndexInvalidated',
    ]));
    for (const type of removedTypes) {
      expect(registered).not.toContain(type);
      expect(isValidEventType(type)).toBe(false);
    }
  });

  it('rejects the removed dataframe schema event type', () => {
    expect(isValidEventType('DataFrameSchemaUpdated')).toBe(false);
  });
});
