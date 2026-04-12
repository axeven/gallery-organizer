import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { QueryClient } from "@tanstack/react-query";
import { useScanStore } from "../store/scanStore";
import { useJobStore } from "../store/jobStore";

// ── Types ────────────────────────────────────────────────────────────────────

export interface Image {
  id: number;
  filePath: string;
  fileName: string;
  fileSizeBytes: number;
  widthPx: number | null;
  heightPx: number | null;
  format: string | null;
  takenAt: number | null;
  takenAtSource: string;
  cameraMake: string | null;
  cameraModel: string | null;
  perceptualHash: string | null;
  scannedAt: number;
  updatedAt: number;
}

export interface Group {
  id: number;
  groupType: string;
  label: string;
  createdAt: number;
  imageCount: number;
  coverImageId: number | null;
}

export interface DuplicateCluster {
  clusterId: number;
  images: Image[];
  suggestedKeeperId: number | null;
}

export interface ProcessingJob {
  id: number;
  status: string;
  operation: string;
  paramsJson: string;
  outputMode: string;
  outputDir: string | null;
  totalImages: number;
  processedCount: number;
  failedCount: number;
  errorMessage: string | null;
  createdAt: number;
  startedAt: number | null;
  finishedAt: number | null;
}

export interface AppSettings {
  outputMode: "output_folder" | "in_place";
  outputDir: string | null;
  defaultQuality: number;
  defaultFormat: "jpeg" | "webp";
  duplicateHashDistance: number;
  dateGroupGranularity: "day" | "month" | "year";
  thumbnailSizePx: number;
  scanRecursive: boolean;
}

export interface ProcessParams {
  quality?: number;
  width?: number;
  height?: number;
  fit?: "contain" | "cover" | "exact";
  targetFormat?: "jpeg" | "webp";
}

export interface PaginatedImages {
  items: Image[];
  total: number;
}

export interface PaginatedJobs {
  items: ProcessingJob[];
  total: number;
}

export interface ScanProgressEvent {
  phase: "walking" | "hashing" | "done";
  scanned: number;
  total: number;
  currentPath: string;
}

export interface JobProgressEvent {
  jobId: number;
  processed: number;
  total: number;
  currentFile: string;
  status: string;
}

// ── Commands ─────────────────────────────────────────────────────────────────

export const scanFolder = (folderPath: string, recursive: boolean) =>
  invoke<void>("scan_folder", { folderPath, recursive });

export const cancelScan = () => invoke<void>("cancel_scan");

export const getImages = (params: {
  groupId?: number;
  page?: number;
  pageSize?: number;
  sortBy?: string;
  sortDir?: string;
}) => invoke<PaginatedImages>("get_images", params);

export const getImageDetail = (imageId: number) =>
  invoke<Image | null>("get_image_detail", { imageId });

export const getThumbnail = (imageId: number) =>
  invoke<string>("get_thumbnail", { imageId });

export const getGroups = (groupType?: string) =>
  invoke<Group[]>("get_groups", { groupType });

export const rebuildGroups = (groupType: "date" | "duplicates" | "all") =>
  invoke<{ groupsCreated: number; durationMs: number }>("rebuild_groups", { groupType });

export const getDuplicateClusters = () =>
  invoke<DuplicateCluster[]>("get_duplicate_clusters");

export const setKeeper = (groupId: number, imageId: number) =>
  invoke<void>("set_keeper", { groupId, imageId });

export const dismissCluster = (groupId: number) =>
  invoke<void>("dismiss_cluster", { groupId });

export const createJob = (payload: {
  imageIds: number[];
  operation: string;
  params: ProcessParams;
  outputMode: string;
  outputDir?: string;
}) => invoke<{ jobId: number }>("create_job", { payload });

export const startJob = (jobId: number) =>
  invoke<void>("start_job", { jobId });

export const cancelJob = (jobId: number) =>
  invoke<void>("cancel_job", { jobId });

export const getJobs = (params: {
  status?: string;
  page?: number;
  pageSize?: number;
}) => invoke<PaginatedJobs>("get_jobs", params);

export const retryFailedImages = (jobId: number) =>
  invoke<{ jobId: number }>("retry_failed_images", { jobId });

export const getSettings = () => invoke<AppSettings>("get_settings");

export const updateSettings = (payload: Partial<AppSettings>) =>
  invoke<AppSettings>("update_settings", { payload });

export const openFolderDialog = () =>
  invoke<string | null>("open_folder_dialog");

export const revealInExplorer = (filePath: string) =>
  invoke<void>("reveal_in_explorer", { filePath });

// ── Event Bridge ──────────────────────────────────────────────────────────────

let unlisteners: UnlistenFn[] = [];

export async function setupEventBridge(queryClient: QueryClient): Promise<void> {
  // Tear down any previous listeners (e.g. HMR)
  teardownEventBridge();

  unlisteners = await Promise.all([
    // Always read from store at event time, not at setup time
    listen<ScanProgressEvent>("scan:progress", (e) => {
      useScanStore.getState().setScanProgress(e.payload);
    }),

    listen("groups:rebuilt", () => {
      queryClient.invalidateQueries({ queryKey: ["groups"] });
      queryClient.invalidateQueries({ queryKey: ["duplicate-clusters"] });
    }),

    listen<JobProgressEvent>("job:progress", (e) => {
      useJobStore.getState().updateJobProgress(e.payload);
    }),

    listen<{ job_id: number }>("job:complete", (e) => {
      useJobStore.getState().clearJob(e.payload.job_id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      queryClient.invalidateQueries({ queryKey: ["images"] });
    }),

    listen<{ job_id: number }>("job:failed", (e) => {
      useJobStore.getState().clearJob(e.payload.job_id);
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
    }),
  ]);
}

export function teardownEventBridge(): void {
  for (const unlisten of unlisteners) {
    unlisten();
  }
  unlisteners = [];
}
