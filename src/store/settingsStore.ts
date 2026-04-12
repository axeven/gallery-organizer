import { create } from "zustand";
import { AppSettings } from "../api/commands";

interface SettingsStore {
  draft: Partial<AppSettings> | null;
  setDraft: (s: Partial<AppSettings>) => void;
  clearDraft: () => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  draft: null,
  setDraft: (s) => set((state) => ({ draft: { ...state.draft, ...s } })),
  clearDraft: () => set({ draft: null }),
}));
