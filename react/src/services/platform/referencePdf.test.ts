import { afterEach, expect, it, vi } from "vitest";
import { loadReferencePdf, MAX_REFERENCE_PDF_BYTES } from "./referencePdf";

afterEach(() => vi.unstubAllGlobals());

it("loads PDF bytes without credentials and retains the requested document URL", async () => {
  const bytes = "%PDF-1.7\nreference content";
  const fetchDocument = vi.fn(
    async () =>
      new Response(bytes, {
        headers: { "content-type": "application/pdf", "x-frame-options": "SAMEORIGIN" },
      }),
  );
  vi.stubGlobal("fetch", fetchDocument);
  const signal = new AbortController().signal;
  const url = "https://example.org/paper.pdf?download=1#page=3";
  const pdf = await loadReferencePdf(url, signal);
  expect(fetchDocument).toHaveBeenCalledWith(url, { credentials: "omit", signal });
  expect(pdf.type).toBe("application/pdf");
  expect(await pdf.text()).toBe(bytes);
});

it("rejects non-PDF responses and cancels oversized or aborted reads before creating a preview", async () => {
  const fetchDocument = vi.fn();
  vi.stubGlobal("fetch", fetchDocument);
  const url = "https://example.org/paper.pdf";
  const signal = new AbortController().signal;
  await expect(loadReferencePdf("javascript:alert(1)", signal)).rejects.toThrow(
    "reference_pdf_invalid_url",
  );
  expect(fetchDocument).not.toHaveBeenCalled();

  fetchDocument.mockResolvedValueOnce(new Response("<html>Login required</html>"));
  await expect(loadReferencePdf(url, signal)).rejects.toThrow("reference_pdf_invalid_content");
  fetchDocument.mockResolvedValueOnce(new Response("Not found", { status: 404 }));
  await expect(loadReferencePdf(url, signal)).rejects.toThrow("reference_pdf_unavailable");

  const cancelled = vi.fn();
  const chunk = new Uint8Array(MAX_REFERENCE_PDF_BYTES / 2 + 1);
  fetchDocument.mockResolvedValueOnce(
    new Response(
      new ReadableStream({
        pull(controller) {
          controller.enqueue(chunk);
        },
        cancel: cancelled,
      }),
    ),
  );
  await expect(loadReferencePdf(url, signal)).rejects.toThrow("reference_pdf_too_large");
  expect(cancelled).toHaveBeenCalledOnce();

  fetchDocument.mockResolvedValueOnce(
    new Response("%PDF-1.7", {
      headers: { "content-length": String(MAX_REFERENCE_PDF_BYTES + 1) },
    }),
  );
  await expect(loadReferencePdf(url, signal)).rejects.toThrow("reference_pdf_too_large");

  const abort = new AbortController();
  abort.abort();
  fetchDocument.mockResolvedValueOnce(new Response("%PDF-1.7"));
  await expect(loadReferencePdf(url, abort.signal)).rejects.toMatchObject({ name: "AbortError" });
});
