import { parseReferenceUrl } from "@/shared/utils/referenceUrl";

// Reference previews retain the complete PDF for the native viewer.
export const MAX_REFERENCE_PDF_BYTES = 64 * 1024 * 1024;

export async function loadReferencePdf(url: string, signal: AbortSignal): Promise<Blob> {
  if (!parseReferenceUrl(url)) throw new Error("reference_pdf_invalid_url");
  const response = await fetch(url, { signal, credentials: "omit" });
  if (!response.ok || !response.body) {
    await response.body?.cancel();
    throw new Error("reference_pdf_unavailable");
  }
  const reader = response.body.getReader();
  const chunks: Uint8Array<ArrayBuffer>[] = [];
  let bytes = 0;
  try {
    if (Number(response.headers.get("content-length")) > MAX_REFERENCE_PDF_BYTES)
      throw new Error("reference_pdf_too_large");
    for (;;) {
      signal.throwIfAborted();
      const { done, value } = await reader.read();
      if (done) break;
      bytes += value.byteLength;
      if (bytes > MAX_REFERENCE_PDF_BYTES) throw new Error("reference_pdf_too_large");
      chunks.push(value);
    }
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
  signal.throwIfAborted();
  const pdf = new Blob(chunks, { type: "application/pdf" });
  // Do not embed an HTML error/login page under a local blob origin. PDF headers
  // may follow a short binary preamble but must occur within the first 1024 bytes.
  const header = await pdf.slice(0, 1024).text();
  if (!header.includes("%PDF-")) throw new Error("reference_pdf_invalid_content");
  signal.throwIfAborted();
  return pdf;
}
