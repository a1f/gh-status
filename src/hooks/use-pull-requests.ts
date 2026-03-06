import { useCallback } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listPullRequests } from "../lib/tauri";
import { useTauriEvent } from "./use-tauri-event";

/**
 * Fetches PRs via Tauri command and auto-invalidates when the
 * Rust sync engine emits a "prs-updated" event.
 * Uses TanStack Query's stale-while-revalidate pattern so the UI
 * shows cached data instantly while refreshing in the background.
 */
export function usePullRequests(accountId?: string) {
  const queryClient = useQueryClient();

  const invalidate = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["pull-requests"] });
  }, [queryClient]);

  useTauriEvent("prs-updated", invalidate);

  return useQuery({
    queryKey: ["pull-requests", accountId],
    queryFn: () => listPullRequests(accountId),
    staleTime: 30_000,
  });
}
