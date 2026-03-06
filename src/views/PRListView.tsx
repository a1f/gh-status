import { usePullRequests } from "../hooks/use-pull-requests";
import { useUIStore } from "../stores/ui-store";
import { PRListItem } from "../components/pr/PRListItem";

export function PRListView() {
  const { data: prs, isLoading } = usePullRequests();
  const { selectedPrId, setSelectedPrId } = useUIStore();

  if (isLoading) {
    return (
      <div className="p-6 text-[var(--text-secondary)]">Loading pull requests...</div>
    );
  }

  if (!prs || prs.length === 0) {
    return (
      <div className="p-6">
        <h2 className="text-lg font-medium mb-2">No pull requests</h2>
        <p className="text-sm text-[var(--text-secondary)]">
          Add a GitHub account and configure repos to watch in Settings.
        </p>
      </div>
    );
  }

  return (
    <div>
      {prs.map((pr) => (
        <PRListItem
          key={pr.id}
          pr={pr}
          selected={selectedPrId === pr.id}
          onSelect={setSelectedPrId}
        />
      ))}
    </div>
  );
}
