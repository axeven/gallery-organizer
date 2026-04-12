import { create } from "zustand";
import { ScanProgressEvent } from "../api/commands";

interface ScanStore {
  isScanning: boolean;
  phase: "idle" | "walking" | "hashing" | "done";
  scanned: number;
  total: number;
  currentPath: string;
  setScanProgress: (e: ScanProgressEvent) => void;
  resetScan: () => void;
}

export const useScanStore = create<ScanStore>((set) => ({
  isScanning: false,
  phase: "idle",
  scanned: 0,
  total: 0,
  currentPath: "",

  setScanProgress: (e) => {
    set({
      isScanning: e.phase !== "done",
      phase: e.phase === "done" ? "done" : e.phase,
      scanned: e.scanned,
      total: e.total,
      currentPath: e.currentPath,
    });
  },

  resetScan: () =>
    set({ isScanning: false, phase: "idle", scanned: 0, total: 0, currentPath: "" }),
}));
