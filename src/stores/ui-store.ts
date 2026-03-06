import { create } from "zustand";
import type { ReviewStatus } from "../types";

export type ActiveView = "dashboard" | "pr-list" | "settings";
export type PrListMode = "grouped" | "flat";

interface PrFilters {
  accountIds: string[];
  repoFilter: string;
  showDrafts: boolean;
  reviewStatus: ReviewStatus | null;
}

interface UIState {
  sidebarWidth: number;
  activeView: ActiveView;
  prListMode: PrListMode;
  selectedPrId: string | null;
  filters: PrFilters;
  setSidebarWidth: (width: number) => void;
  setActiveView: (view: ActiveView) => void;
  setPrListMode: (mode: PrListMode) => void;
  setSelectedPrId: (id: string | null) => void;
  setFilters: (filters: Partial<PrFilters>) => void;
}

export const useUIStore = create<UIState>((set) => ({
  sidebarWidth: 260,
  activeView: "pr-list",
  prListMode: "grouped",
  selectedPrId: null,
  filters: {
    accountIds: [],
    repoFilter: "",
    showDrafts: true,
    reviewStatus: null,
  },
  setSidebarWidth: (width) => set({ sidebarWidth: width }),
  setActiveView: (view) => set({ activeView: view }),
  setPrListMode: (mode) => set({ prListMode: mode }),
  setSelectedPrId: (id) => set({ selectedPrId: id }),
  setFilters: (partial) =>
    set((state) => ({ filters: { ...state.filters, ...partial } })),
}));
