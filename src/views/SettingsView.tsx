import { useCallback, useEffect, useState } from "react";
import { listAccounts, addAccount, removeAccount } from "../lib/tauri";
import type { Account } from "../types";

export function SettingsView() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [token, setToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [removingId, setRemovingId] = useState<string | null>(null);

  const fetchAccounts = useCallback(async () => {
    try {
      const result = await listAccounts();
      setAccounts(result);
    } catch (e) {
      setError(extractError(e));
    }
  }, []);

  useEffect(() => {
    fetchAccounts();
  }, [fetchAccounts]);

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = token.trim();
    if (!trimmed) return;

    setError(null);
    setLoading(true);
    try {
      await addAccount(trimmed);
      setToken("");
      await fetchAccounts();
    } catch (e) {
      setError(extractError(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleRemove(id: string) {
    setError(null);
    setRemovingId(id);
    try {
      await removeAccount(id);
      await fetchAccounts();
    } catch (e) {
      setError(extractError(e));
    } finally {
      setRemovingId(null);
    }
  }

  return (
    <div className="p-6 max-w-xl">
      <h2 className="text-lg font-medium mb-6">Settings</h2>

      {/* Add Account Form */}
      <section className="mb-8">
        <h3 className="text-sm font-medium mb-3 text-[var(--text-secondary)]">
          Add GitHub Account
        </h3>
        <form onSubmit={handleAdd} className="flex gap-2">
          <input
            type="password"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="GitHub Personal Access Token"
            className="flex-1 px-3 py-1.5 text-sm rounded border border-[var(--border)] bg-[var(--bg-primary)] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:ring-1 focus:ring-[var(--accent)]"
            disabled={loading}
          />
          <button
            type="submit"
            disabled={loading || !token.trim()}
            className="px-4 py-1.5 text-sm rounded bg-[var(--accent)] text-white font-medium disabled:opacity-50 hover:opacity-90"
          >
            {loading ? "Validating…" : "Add"}
          </button>
        </form>
        {error && (
          <p className="mt-2 text-sm text-red-400">{error}</p>
        )}
      </section>

      {/* Account List */}
      <section>
        <h3 className="text-sm font-medium mb-3 text-[var(--text-secondary)]">
          Accounts
        </h3>
        {accounts.length === 0 ? (
          <p className="text-sm text-[var(--text-tertiary)]">
            No accounts configured. Add a GitHub PAT above to get started.
          </p>
        ) : (
          <ul className="space-y-2">
            {accounts.map((account) => (
              <li
                key={account.id}
                className="flex items-center gap-3 p-3 rounded border border-[var(--border)] bg-[var(--bg-secondary)]"
              >
                <img
                  src={account.avatarUrl}
                  alt={account.username}
                  className="w-8 h-8 rounded-full"
                />
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">
                    {account.username}
                  </p>
                  {account.orgs.length > 0 && (
                    <p className="text-xs text-[var(--text-tertiary)] truncate">
                      {account.orgs.join(", ")}
                    </p>
                  )}
                </div>
                <button
                  onClick={() => handleRemove(account.id)}
                  disabled={removingId === account.id}
                  className="px-2 py-1 text-xs rounded text-red-400 hover:bg-red-400/10 disabled:opacity-50"
                >
                  {removingId === account.id ? "Removing…" : "Remove"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function extractError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return "An unexpected error occurred";
}
