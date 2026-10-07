import { useEffect, useRef, useState } from "react";
import { CatalogService, type NodeCreationForm } from "@/services/nodeSystem/catalogService";
import { captureProjectReadContext } from "@/features/application/project/projectIOStore";

export function useNodeCreationForm(projectInstanceId: string, nodeTypeId: string, locale: string) {
  const [form, setForm] = useState<NodeCreationForm | null>(null);
  const [loadError, setLoadError] = useState<unknown>(null);
  const [errors, setErrors] = useState<Record<string, unknown>>({});
  const [pendingKeys, setPendingKeys] = useState<ReadonlySet<string>>(new Set());
  const session = useRef<{
    isCurrent(): boolean;
    form: NodeCreationForm | null;
    errors: Record<string, unknown>;
    queue: Promise<void>;
    pending: Set<string>;
  } | null>(null);

  useEffect(() => {
    const context = captureProjectReadContext(projectInstanceId);
    if (!context) return;
    let active = true;
    const current = {
      isCurrent: () => active && context.isCurrent(),
      form: null as NodeCreationForm | null,
      errors: {} as Record<string, unknown>,
      pending: new Set<string>(),
      queue: Promise.resolve(),
    };
    session.current = current;
    current.queue = CatalogService.getNodeCreationForm(
      projectInstanceId,
      nodeTypeId,
      {},
      {},
      locale,
    )
      .then((response) => {
        if (!current.isCurrent()) return;
        current.form = response;
        setForm(response);
      })
      .catch((error: unknown) => {
        if (current.isCurrent()) setLoadError(error);
      });
    return () => {
      active = false;
    };
  }, [projectInstanceId, nodeTypeId, locale]);

  const commit = (
    key: string,
    change: (form: NodeCreationForm) => {
      parameters: Record<string, unknown>;
      portCounts: Record<string, number>;
    },
    onResolved?: () => void,
  ) => {
    const current = session.current;
    if (!current?.isCurrent() || !current.form || current.pending.has(key)) return;
    current.pending.add(key);
    setPendingKeys(new Set(current.pending));
    // Different fields can finish editing before the prior preview returns. Serialize
    // against the latest canonical values so neither edit overwrites the other.
    current.queue = current.queue.then(async () => {
      if (!current.isCurrent() || !current.form) return;
      try {
        const next = change(current.form);
        const response = await CatalogService.getNodeCreationForm(
          projectInstanceId,
          nodeTypeId,
          next.parameters,
          next.portCounts,
          locale,
        );
        if (!current.isCurrent()) return;
        current.form = response;
        delete current.errors[key];
        const visibleParameters = new Set(
          response.groups.flatMap((group) =>
            group.parameters.map((parameter) => `parameter:${parameter.key}`),
          ),
        );
        for (const field of Object.keys(current.errors)) {
          if (field.startsWith("parameter:") && !visibleParameters.has(field))
            delete current.errors[field];
        }
        setForm(response);
        onResolved?.();
      } catch (error) {
        if (current.isCurrent()) current.errors[key] = error;
      } finally {
        current.pending.delete(key);
        if (current.isCurrent()) {
          setErrors({ ...current.errors });
          setPendingKeys(new Set(current.pending));
        }
      }
    });
  };

  const preparedValues = async () => {
    const current = session.current;
    if (!current) return null;
    let pending;
    do {
      pending = current.queue;
      await pending;
    } while (pending !== current.queue);
    if (!current.isCurrent() || Object.keys(current.errors).length > 0) return null;
    return current.form;
  };

  const commitParameter = (key: string, value: unknown, onResolved?: () => void) =>
    commit(
      `parameter:${key}`,
      (form) => ({ parameters: { ...form.values, [key]: value }, portCounts: form.portCounts }),
      onResolved,
    );

  const commitPortCount = (templates: readonly string[], count: number) =>
    commit(`port:${templates[0]}`, (form) => ({
      parameters: form.values,
      portCounts: {
        ...form.portCounts,
        ...Object.fromEntries(templates.map((key) => [key, count])),
      },
    }));

  return { form, loadError, errors, pendingKeys, commitParameter, commitPortCount, preparedValues };
}
