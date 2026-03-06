import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

/**
 * Subscribe to a Tauri event emitted from Rust.
 * Automatically cleans up the listener on unmount.
 * This is the core reactivity bridge: Rust sync engine emits events,
 * React hooks invalidate TanStack Query caches in response.
 */
export function useTauriEvent<T>(event: string, handler: (payload: T) => void) {
  useEffect(() => {
    const unlisten = listen<T>(event, (e) => handler(e.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [event, handler]);
}
