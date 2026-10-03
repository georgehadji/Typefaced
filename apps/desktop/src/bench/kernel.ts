import initKernel from "@typefaced/geometry-wasm";
// `?inline` makes Vite embed the module as a base64 data URL in the bundle. The default
// loader would fetch() the file, and the shipped CSP's connect-src has no `'self'`.
import wasmDataUrl from "@typefaced/geometry-wasm/wasm?inline";

/** Decodes a `data:...;base64,` URL to its bytes. */
export function dataUrlBytes(url: string): Uint8Array {
  const marker = ";base64,";
  const at = url.indexOf(marker);
  if (!url.startsWith("data:") || at < 0) {
    throw new TypeError("expected a base64 data URL");
  }
  return Uint8Array.from(atob(url.slice(at + marker.length)), (c) =>
    c.charCodeAt(0),
  );
}

let kernelReady: Promise<unknown> | undefined;

/** Instantiates the WASM kernel once, from the inlined bytes (no network request). */
export function loadKernel(): Promise<unknown> {
  kernelReady ??= initKernel({ module_or_path: dataUrlBytes(wasmDataUrl) });
  return kernelReady;
}
