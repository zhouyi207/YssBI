import { beforeEach, describe, expect, it, vi } from 'vitest';
import { registerCoreApplicationPorts } from './registerCoreApplicationPorts';
import { initializeApplication } from './initializeApplication';

vi.mock('./registerCoreApplicationPorts', () => ({
  registerCoreApplicationPorts: vi.fn(),
}));

describe('initializeApplication', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('registers application adaptors for core ports', () => {
    initializeApplication();

    expect(registerCoreApplicationPorts).toHaveBeenCalledOnce();
  });
});
