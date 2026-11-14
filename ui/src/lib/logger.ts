// Wires the frontend into the same log file/stdout the Rust backend writes
// to (see src-tauri/src/lib.rs build_logger()). Two directions:
//
// 1. attachConsole() - Rust-side log::info!/warn!/etc. records also print to
//    this window's DevTools console (requires devtools open to see).
// 2. Global error/rejection handlers - every uncaught JS exception and
//    unhandled promise rejection gets forwarded into the Rust log file via
//    error(), so a frontend crash leaves a trail on disk even if nobody had
//    DevTools open at the time. This is the piece that would have surfaced
//    the "Operation was cancelled" resource-load failure's downstream
//    effects (e.g. the listProfiles() rejection in App.svelte's onMount).
import { attachConsole, error as logError } from "@tauri-apps/plugin-log";

let attached = false;

export async function initLogger(): Promise<void> {
  if (attached) return;
  attached = true;

  await attachConsole();

  window.addEventListener("error", (event) => {
    void logError(`Uncaught error: ${event.message} at ${event.filename}:${event.lineno}`);
  });

  window.addEventListener("unhandledrejection", (event) => {
    const reason =
      event.reason instanceof Error ? event.reason.stack ?? event.reason.message : String(event.reason);
    void logError(`Unhandled promise rejection: ${reason}`);
  });
}
