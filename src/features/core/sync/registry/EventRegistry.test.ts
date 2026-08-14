import { describe, expect, it } from 'vitest';
import { createEventHandlers } from '../handlers';
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

  it('preserves unrelated project, resource, database, and variable events', () => {
    const registered = new EventRegistry(createEventHandlers()).getRegisteredTypes();

    expect(registered).toEqual(expect.arrayContaining([
      'ProjectLoaded',
      'ProjectCleared',
      'ProjectLifecycleCommitted',
      'ProjectSaved',
      'ResourceChanged',
      'ProjectIndexInvalidated',
      'DataFrameCreated',
      'DataFrameDeleted',
      'DataFrameSchemaUpdated',
      'VariableCreated',
      'VariableUpdated',
      'VariableDeleted',
    ]));
  });
});
