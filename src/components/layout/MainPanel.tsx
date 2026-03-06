import { useUIStore } from "../../stores/ui-store";
import { PRListView } from "../../views/PRListView";
import { SettingsView } from "../../views/SettingsView";

export function MainPanel() {
  const activeView = useUIStore((s) => s.activeView);

  return (
    <main className="flex-1 overflow-auto">
      {activeView === "pr-list" && <PRListView />}
      {activeView === "settings" && <SettingsView />}
      {activeView === "dashboard" && (
        <div className="p-6 text-[var(--text-secondary)]">
          Dashboard coming in Project 2.
        </div>
      )}
    </main>
  );
}
