import type { PullRequest } from "../../types";
import { cn } from "../../lib/utils";

interface Props {
  pr: PullRequest;
  selected: boolean;
  onSelect: (id: string) => void;
}

export function PRListItem({ pr, selected, onSelect }: Props) {
  return (
    <button
      onClick={() => onSelect(pr.id)}
      className={cn(
        "w-full text-left px-3 py-2 border-b border-[var(--border)] text-sm",
        selected ? "bg-[var(--bg-tertiary)]" : "hover:bg-[var(--bg-secondary)]"
      )}
    >
      <div className="flex items-center gap-2">
        <span className="font-medium truncate">{pr.title}</span>
        <span className="text-[var(--text-secondary)] shrink-0">#{pr.number}</span>
      </div>
      <div className="text-xs text-[var(--text-secondary)] mt-0.5">
        {pr.repo.owner}/{pr.repo.name}
      </div>
    </button>
  );
}
