import { describe, expect, it } from 'vitest';
import { createEventHandlers } from '../handlers';
import { EventRegistry } from './EventRegistry';

const legacyGraphMutationEvents = [
  'NodeCreated',
  'NodesBatchCreated',
  'NodeUpdated',
  'NodeDeleted',
  'NodesBatchDeleted',
  'NodePositionsUpdated',
  'NodePinsUpdated',
  'PinTypesInferred',
  'RuntimeSourcesInvalidated',
  'ConnectionCreated',
  'ConnectionDeleted',
  'ConnectionsBatchDeleted',
  'ConnectionsBatchCreated',
];

describe('EventRegistry project mutation registrations', () => {
  it('registers revisioned events and removes legacy graph mutation events', () => {
    const registered = new EventRegistry(createEventHandlers()).getRegisteredTypes();

    expect(registered).toEqual(expect.arrayContaining([
      'GraphDelta',
      'ResourceMutationCommitted',
      'ComputationSettingsChanged',
    ]));
    expect(registered).not.toEqual(expect.arrayContaining(legacyGraphMutationEvents));
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
    expect(registered).not.toContain('GraphResourceMoved');
  });
});
