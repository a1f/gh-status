import { useUIStore, type ActiveView } from "../../stores/ui-store";
import { cn } from "../../lib/utils";

const NAV_ITEMS: { id: ActiveView; label: string }[] = [
  { id: "pr-list", label: "Pull Requests" },
  { id: "settings", label: "Settings" },
];

export function Sidebar() {
  const { activeView, setActiveView, sidebarWidth } = useUIStore();

  return (
    <aside
      className="flex flex-col border-r border-[var(--border)] bg-[var(--bg-secondary)]"
      style={{ width: sidebarWidth }}
    >
      <div className="p-4 border-b border-[var(--border)]">
        <h1 className="text-lg font-semibold">gh-status</h1>
        <p className="text-xs text-[var(--text-secondary)]">PR Dashboard</p>
      </div>

      <nav className="flex-1 p-2">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.id}
            onClick={() => setActiveView(item.id)}
            className={cn(
              "w-full text-left px-3 py-2 rounded text-sm",
              activeView === item.id
                ? "bg-[var(--bg-tertiary)] text-[var(--text-primary)]"
                : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
            )}
          >
            {item.label}
          </button>
        ))}
      </nav>
    </aside>
  );
}
