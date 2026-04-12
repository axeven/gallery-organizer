import { create } from "zustand";
import { JobProgressEvent } from "../api/commands";

interface JobStore {
  activeJobProgress: Record<number, JobProgressEvent>;
  updateJobProgress: (e: JobProgressEvent) => void;
  clearJob: (jobId: number) => void;
}

export const useJobStore = create<JobStore>((set) => ({
  activeJobProgress: {},

  updateJobProgress: (e) =>
    set((state) => ({
      activeJobProgress: { ...state.activeJobProgress, [e.jobId]: e },
    })),

  clearJob: (jobId) =>
    set((state) => {
      const next = { ...state.activeJobProgress };
      delete next[jobId];
      return { activeJobProgress: next };
    }),
}));
