export function StatusBar() {
  return (
    <footer className="flex items-center justify-between px-4 py-1 border-t border-[var(--border)] bg-[var(--bg-secondary)] text-xs text-[var(--text-secondary)]">
      <span>gh-status v0.1.0</span>
      <span>Last synced: never</span>
    </footer>
  );
}
