export interface ProjectEventIngress<T> {
  readonly push: (event: T) => boolean;
  readonly close: () => void;
}

export function createProjectEventIngress<T>(
  capacity: number,
  consume: (event: T) => void,
): ProjectEventIngress<T> {
  const queue: T[] = [];
  let closed = false;
  return {
    push(event) {
      if (closed || queue.length >= capacity) return false;
      queue.push(event);
      const next = queue.shift();
      if (next !== undefined) consume(next);
      return true;
    },
    close() {
      closed = true;
      queue.length = 0;
    },
  };
}
