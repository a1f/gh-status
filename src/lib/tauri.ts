import { invoke } from "@tauri-apps/api/core";
import type { Account, PullRequest } from "../types";

/** Typed wrappers around Tauri invoke() to keep IPC calls type-safe */

export async function listAccounts(): Promise<Account[]> {
  return invoke("list_accounts");
}

export async function addAccount(token: string): Promise<Account> {
  return invoke("add_account", { token });
}

export async function removeAccount(id: string): Promise<void> {
  return invoke("remove_account", { id });
}

export async function listPullRequests(accountId?: string): Promise<PullRequest[]> {
  return invoke("list_pull_requests", { accountId: accountId ?? null });
}

export async function triggerSync(accountId?: string): Promise<void> {
  return invoke("trigger_sync", { accountId: accountId ?? null });
}
