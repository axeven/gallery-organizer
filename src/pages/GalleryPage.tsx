import { useState, useEffect, useCallback } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getGroups, rebuildGroups, getThumbnail, getImages, removeGroup, removeImageFromGroup, trashImage, getFullImage, getGroupSummary, processGroup, openFolderDialog } from "../api/commands";
import type { Group, Image, ResizeMode, ImageSummaryItem } from "../api/commands";

type Granularity = "day" | "month" | "year";

// ── Lightbox ──────────────────────────────────────────────────────────────────

function Lightbox({ image, onClose }: { image: Image; onClose: () => void }) {
  const { data, isFetching } = useQuery({
    queryKey: ["full-image", image.id],
    queryFn: () => getFullImage(image.id),
    staleTime: Infinity,
  });

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  const src = data ? `data:${data[1]};base64,${data[0]}` : null;

  return (
    <div
      className="fixed inset-0 z-50 bg-black/90 flex flex-col items-center justify-center"
      onClick={onClose}
    >
      {/* Close button */}
      <button
        className="absolute top-4 right-4 text-white/70 hover:text-white text-2xl leading-none"
        onClick={onClose}
      >
        ✕
      </button>

      {/* Image */}
      <div
        className="relative max-w-[90vw] max-h-[85vh] flex items-center justify-center"
        onClick={(e) => e.stopPropagation()}
      >
        {isFetching && (
          <div className="w-32 h-32 flex items-center justify-center">
            <div className="w-8 h-8 border-2 border-white/30 border-t-white rounded-full animate-spin" />
          </div>
        )}
        {src && (
          <img
            src={src}
            className="max-w-[90vw] max-h-[85vh] object-contain rounded shadow-2xl"
          />
        )}
      </div>

      {/* Metadata bar */}
      <div
        className="absolute bottom-0 inset-x-0 px-6 py-3 bg-black/60 flex items-center gap-4 text-xs text-neutral-300"
        onClick={(e) => e.stopPropagation()}
      >
        <span className="truncate text-neutral-400 max-w-xs" title={image.filePath}>
          {image.fileName}
        </span>
        {image.widthPx && image.heightPx && (
          <span className="shrink-0">{image.widthPx} × {image.heightPx}</span>
        )}
        {image.fileSizeBytes && (
          <span className="shrink-0">{(image.fileSizeBytes / 1024 / 1024).toFixed(1)} MB</span>
        )}
        {image.cameraMake && (
          <span className="shrink-0 text-neutral-500">{image.cameraMake} {image.cameraModel}</span>
        )}
      </div>
    </div>
  );
}

type ImageAction = "remove" | "trash";

function ImageThumb({
  image,
  groupId,
  onRemoved,
  onTrashed,
  onOpen,
}: {
  image: Image;
  groupId: number;
  onRemoved: (imageId: number) => void;
  onTrashed: (imageId: number) => void;
  onOpen: (image: Image) => void;
}) {
  const queryClient = useQueryClient();
  const [confirm, setConfirm] = useState<ImageAction | null>(null);

  const { data: src } = useQuery({
    queryKey: ["thumbnail", image.id],
    queryFn: () => getThumbnail(image.id),
    staleTime: Infinity,
  });

  const removeMutation = useMutation({
    mutationFn: () => removeImageFromGroup(groupId, image.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["groups"] });
      onRemoved(image.id);
    },
  });

  const trashMutation = useMutation({
    mutationFn: () => trashImage(image.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["groups"] });
      queryClient.removeQueries({ queryKey: ["thumbnail", image.id] });
      onTrashed(image.id);
    },
  });

  const isPending = removeMutation.isPending || trashMutation.isPending;

  const res =
    image.widthPx && image.heightPx
      ? `${image.widthPx}×${image.heightPx}`
      : null;

  return (
    <div className="relative aspect-square bg-neutral-800 rounded overflow-hidden group/thumb">
      {src ? (
        <img
          src={`data:image/jpeg;base64,${src}`}
          className="w-full h-full object-cover cursor-pointer"
          loading="lazy"
          onClick={() => onOpen(image)}
        />
      ) : (
        <div className="w-full h-full animate-pulse bg-neutral-700" />
      )}
      {res && (
        <span className="absolute bottom-0 inset-x-0 text-center text-[9px] leading-tight py-0.5 bg-black/60 text-neutral-300 truncate">
          {res}
        </span>
      )}
      {/* Action buttons — visible on hover */}
      {!confirm && (
        <div className="absolute top-1 right-1 flex gap-0.5 opacity-0 group-hover/thumb:opacity-100 transition-opacity">
          <button
            onClick={(e) => { e.stopPropagation(); setConfirm("remove"); }}
            className="bg-black/70 hover:bg-neutral-600 text-white rounded text-[10px] px-1 py-0.5 leading-none"
            title="Remove from group"
          >
            ✕
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); setConfirm("trash"); }}
            className="bg-black/70 hover:bg-red-700 text-white rounded text-[10px] px-1 py-0.5 leading-none"
            title="Move to trash"
          >
            🗑
          </button>
        </div>
      )}
      {confirm && (
        <div className="absolute inset-0 bg-black/80 flex flex-col items-center justify-center gap-1.5 p-1">
          <span className="text-[10px] text-center text-white leading-tight">
            {confirm === "trash" ? "Move to trash?" : "Remove from group?"}
          </span>
          <div className="flex gap-1">
            <button
              onClick={(e) => {
                e.stopPropagation();
                confirm === "trash" ? trashMutation.mutate() : removeMutation.mutate();
              }}
              disabled={isPending}
              className={`text-[10px] px-1.5 py-0.5 rounded text-white disabled:opacity-40 ${
                confirm === "trash"
                  ? "bg-red-600 hover:bg-red-500"
                  : "bg-neutral-600 hover:bg-neutral-500"
              }`}
            >
              Yes
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); setConfirm(null); }}
              className="text-[10px] px-1.5 py-0.5 bg-neutral-700 hover:bg-neutral-600 rounded text-white"
            >
              No
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

const PAGE_SIZE = 20;

type SortBy = "taken_at" | "perceptual_hash" | "file_name" | "file_size";

const SORT_LABELS: Record<SortBy, string> = {
  taken_at: "Date",
  perceptual_hash: "Similarity",
  file_name: "Name",
  file_size: "Size",
};

// ── Process Summary Modal ─────────────────────────────────────────────────────

function fmtMB(bytes: number) {
  return (bytes / 1024 / 1024).toFixed(1) + " MB";
}

/** Estimate output pixel count for a given resize config. */
function estimateOutputDims(
  w: number | null,
  h: number | null,
  resize: ResizeMode
): [number, number] | null {
  if (!w || !h) return null;
  if (resize.mode === "none") return [w, h];
  if (resize.mode === "half") return [Math.floor(w / 2), Math.floor(h / 2)];
  // fixed — orientation-aware swap
  let tw = resize.width;
  let th = resize.height;
  const origPortrait = h > w;
  const targetPortrait = th > tw;
  if (origPortrait !== targetPortrait) { [tw, th] = [th, tw]; }
  const scale = Math.min(tw / w, th / h, 1.0);
  return [Math.round(w * scale), Math.round(h * scale)];
}

/** Very rough JPEG byte estimate: quality * pixels * 0.1 / 8 (empirical). */
function estimateFileSize(w: number, h: number, quality: number): number {
  return Math.round(w * h * quality * 0.012);
}

interface ProcessConfig {
  outputMode: "folder" | "overwrite";
  outputDir: string;
  moveFiles: boolean;
  resize: ResizeMode;
  targetFormat: "jpeg" | "webp";
  quality: number;
}

function ProcessSummaryModal({
  group,
  config,
  items,
  onConfirm,
  onCancel,
}: {
  group: Group;
  config: ProcessConfig;
  items: ImageSummaryItem[];
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const totalOrigBytes = items.reduce((s, i) => s + i.fileSizeBytes, 0);

  const estimatedBytes = items.reduce((s, i) => {
    const dims = estimateOutputDims(i.widthPx, i.heightPx, config.resize);
    if (!dims) return s + i.fileSizeBytes;
    return s + estimateFileSize(dims[0], dims[1], config.quality);
  }, 0);

  const saving = totalOrigBytes - estimatedBytes;
  const savingPct = totalOrigBytes > 0 ? Math.round((saving / totalOrigBytes) * 100) : 0;

  const resizeLabel = () => {
    if (config.resize.mode === "none") return "No resize (format/quality only)";
    if (config.resize.mode === "half") return "Scale to 50% of original dimensions";
    return `Fit within ${config.resize.width}×${config.resize.height} px (orientation-aware, never upscale)`;
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-6">
      <div
        className="bg-neutral-900 border border-neutral-700 rounded-xl w-full max-w-md flex flex-col gap-4 p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-sm font-semibold">Confirm Processing</h2>

        <div className="flex flex-col gap-1.5 text-xs text-neutral-300">
          <div className="flex justify-between">
            <span className="text-neutral-500">Group</span>
            <span className="font-medium text-blue-400">{group.label}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-neutral-500">Images</span>
            <span>{items.length}</span>
          </div>
          {config.outputMode === "overwrite" ? (
            <div className="flex justify-between">
              <span className="text-neutral-500">Output</span>
              <span className="text-yellow-400">Overwrite originals in place</span>
            </div>
          ) : (
            <div className="flex justify-between">
              <span className="text-neutral-500">Output folder</span>
              <span className="truncate max-w-[220px] text-right" title={config.outputDir}>
                {config.outputDir}/{group.label}
              </span>
            </div>
          )}
          {config.outputMode === "folder" && (
            <div className="flex justify-between">
              <span className="text-neutral-500">Action</span>
              <span>{config.moveFiles ? "Move files" : "Copy files"}</span>
            </div>
          )}
          <div className="flex justify-between">
            <span className="text-neutral-500">Resize</span>
            <span className="text-right max-w-[220px]">{resizeLabel()}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-neutral-500">Format / Quality</span>
            <span>{config.targetFormat.toUpperCase()} @ {config.quality}%</span>
          </div>
        </div>

        <div className="border-t border-neutral-800 pt-3 flex flex-col gap-1 text-xs">
          <div className="flex justify-between text-neutral-400">
            <span>Total original size</span>
            <span>{fmtMB(totalOrigBytes)}</span>
          </div>
          <div className="flex justify-between text-neutral-400">
            <span>Estimated output size</span>
            <span>{fmtMB(estimatedBytes)}</span>
          </div>
          <div className={`flex justify-between font-medium ${saving > 0 ? "text-green-400" : "text-neutral-300"}`}>
            <span>Estimated saving</span>
            <span>
              {saving > 0 ? `~${fmtMB(saving)} (${savingPct}% smaller)` : "~no saving (already small)"}
            </span>
          </div>
          <p className="text-[10px] text-neutral-600 mt-0.5">
            Size estimate is approximate — actual results vary by image content.
          </p>
        </div>

        <div className="flex gap-2 pt-1">
          <button
            onClick={onConfirm}
            className="flex-1 py-2 bg-blue-600 hover:bg-blue-500 rounded text-sm font-medium transition-colors"
          >
            Start Job
          </button>
          <button
            onClick={onCancel}
            className="flex-1 py-2 bg-neutral-700 hover:bg-neutral-600 rounded text-sm transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Group Card ────────────────────────────────────────────────────────────────

function GroupCard({ group, onRemoved, onOpenImage }: { group: Group; onRemoved: (groupId: number) => void; onOpenImage: (image: Image) => void }) {
  const [expanded, setExpanded] = useState(false);
  const [page, setPage] = useState(0);
  const [allImages, setAllImages] = useState<Image[]>([]);
  // serverTotal: last total reported by the server (updated on each page fetch)
  // removedCount: how many images have been removed/trashed locally since last reset
  const [serverTotal, setServerTotal] = useState<number>(group.imageCount ?? 0);
  const [removedCount, setRemovedCount] = useState(0);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [showProcess, setShowProcess] = useState(false);
  const [sortBy, setSortBy] = useState<SortBy>("taken_at");

  // Process panel state
  const [outputMode, setOutputMode] = useState<"folder" | "overwrite">("folder");
  const [outputDir, setOutputDir] = useState("");
  const [moveFiles, setMoveFiles] = useState(false);
  const [resizeMode, setResizeMode] = useState<"none" | "half" | "fixed">("none");
  const [fixedW, setFixedW] = useState(1280);
  const [fixedH, setFixedH] = useState(720);
  const [targetFormat, setTargetFormat] = useState<"jpeg" | "webp">("jpeg");
  const [quality, setQuality] = useState(85);
  const [showSummary, setShowSummary] = useState(false);
  const [summaryItems, setSummaryItems] = useState<ImageSummaryItem[]>([]);

  const queryClient = useQueryClient();

  const total = serverTotal - removedCount;

  const { data: pageData, isFetching } = useQuery({
    queryKey: ["images", group.id, page, sortBy],
    queryFn: () => getImages({ groupId: group.id, page, pageSize: PAGE_SIZE, sortBy }),
    enabled: expanded,
    staleTime: Infinity,
  });

  useEffect(() => {
    if (!pageData) return;
    setAllImages((prev) => {
      const existingIds = new Set(prev.map((img) => img.id));
      const newItems = pageData.items.filter((img) => !existingIds.has(img.id));
      return newItems.length > 0 ? [...prev, ...newItems] : prev;
    });
    setServerTotal(pageData.total);
    setRemovedCount(0);
  }, [pageData]);

  // Reset accumulated images when sort changes
  useEffect(() => {
    setAllImages([]);
    setPage(0);
    setRemovedCount(0);
  }, [sortBy]);

  const removeGroupMutation = useMutation({
    mutationFn: () => removeGroup(group.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["groups"] });
      onRemoved(group.id);
    },
  });

  const processMutation = useMutation({
    mutationFn: () => {
      const resize: ResizeMode =
        resizeMode === "half"
          ? { mode: "half" }
          : resizeMode === "fixed"
          ? { mode: "fixed", width: fixedW, height: fixedH }
          : { mode: "none" };
      return processGroup(group.id, {
        outputMode,
        outputDir: outputMode === "folder" ? outputDir : undefined,
        moveFiles,
        resize,
        targetFormat,
        quality,
      });
    },
    onSuccess: () => {
      // refetchType: "all" ensures the Jobs page cache is refreshed even if it's
      // not currently mounted (process_group starts the job immediately, so by
      // the time the user navigates to Jobs it may already be done).
      queryClient.invalidateQueries({ queryKey: ["jobs"], refetchType: "all" });
      setShowProcess(false);
      setShowSummary(false);
    },
  });

  const handlePreview = async () => {
    if (outputMode === "folder" && !outputDir) return;
    const items = await getGroupSummary(group.id);
    setSummaryItems(items);
    setShowSummary(true);
  };

  const handleImageRemoved = (imageId: number) => {
    setAllImages((prev) => prev.filter((img) => img.id !== imageId));
    setRemovedCount((c) => c + 1);
  };

  const handleImageTrashed = (imageId: number) => {
    setAllImages((prev) => prev.filter((img) => img.id !== imageId));
    setRemovedCount((c) => c + 1);
  };

  const hasMore = allImages.length < total;

  const currentResize: ResizeMode =
    resizeMode === "half"
      ? { mode: "half" }
      : resizeMode === "fixed"
      ? { mode: "fixed", width: fixedW, height: fixedH }
      : { mode: "none" };

  return (
    <div className="bg-neutral-900 rounded-lg overflow-hidden border border-neutral-800">
      {showSummary && summaryItems.length > 0 && (
        <ProcessSummaryModal
          group={group}
          config={{ outputMode, outputDir, moveFiles, resize: currentResize, targetFormat, quality }}
          items={summaryItems}
          onConfirm={() => processMutation.mutate()}
          onCancel={() => setShowSummary(false)}
        />
      )}
      <div className="flex items-center">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex-1 flex items-center justify-between px-4 py-3 hover:bg-neutral-800 transition-colors text-left"
        >
          <div className="flex items-baseline gap-2">
            <span className="font-medium text-sm">{group.label}</span>
            <span className="text-xs font-medium text-neutral-400 bg-neutral-700 px-1.5 py-0.5 rounded">
              {total}
            </span>
          </div>
          <span className="text-neutral-500 text-xs">{expanded ? "▲" : "▼"}</span>
        </button>
        {/* Process button */}
        <button
          onClick={() => { setShowProcess((v) => !v); setConfirmRemove(false); }}
          className={`px-3 py-3 text-xs transition-colors ${showProcess ? "text-blue-400" : "text-neutral-500 hover:text-blue-400"}`}
          title="Process group (resize/export)"
        >
          ↗
        </button>
        {/* Group-level remove */}
        {!confirmRemove ? (
          <button
            onClick={() => { setConfirmRemove(true); setShowProcess(false); }}
            className="px-3 py-3 text-neutral-500 hover:text-red-400 transition-colors text-sm"
            title="Remove group"
          >
            ✕
          </button>
        ) : (
          <div className="flex items-center gap-1 px-2">
            <span className="text-xs text-neutral-400">Remove group?</span>
            <button
              onClick={() => removeGroupMutation.mutate()}
              disabled={removeGroupMutation.isPending}
              className="text-xs px-2 py-1 bg-red-600 hover:bg-red-500 rounded text-white disabled:opacity-40"
            >
              Yes
            </button>
            <button
              onClick={() => setConfirmRemove(false)}
              className="text-xs px-2 py-1 bg-neutral-700 hover:bg-neutral-600 rounded text-white"
            >
              No
            </button>
          </div>
        )}
      </div>

      {showProcess && (
        <div className="px-4 py-3 border-t border-neutral-800 bg-neutral-950 flex flex-col gap-3">
          <p className="text-xs font-medium text-neutral-300">
            Process <span className="text-blue-400">{group.label}</span>
          </p>

          {/* Output mode toggle */}
          <div className="flex flex-col gap-1.5">
            <label className="text-[11px] text-neutral-500">Destination</label>
            <div className="flex gap-1.5">
              {(["folder", "overwrite"] as const).map((m) => (
                <button
                  key={m}
                  onClick={() => setOutputMode(m)}
                  className={`text-[11px] px-2.5 py-1 rounded transition-colors ${
                    outputMode === m
                      ? "bg-blue-600 text-white"
                      : "bg-neutral-800 text-neutral-400 hover:text-white"
                  }`}
                >
                  {m === "folder" ? "New folder" : "Overwrite originals"}
                </button>
              ))}
            </div>
          </div>

          {outputMode === "folder" && (
            <>
              {/* Output folder picker */}
              <div className="flex flex-col gap-1">
                <label className="text-[11px] text-neutral-500">Output folder</label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={outputDir}
                    readOnly
                    placeholder="Select output folder…"
                    className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-2 py-1.5 text-xs text-neutral-200 placeholder-neutral-500 cursor-pointer"
                    onClick={async () => {
                      const path = await openFolderDialog();
                      if (path) setOutputDir(path);
                    }}
                  />
                  <button
                    onClick={async () => {
                      const path = await openFolderDialog();
                      if (path) setOutputDir(path);
                    }}
                    className="px-3 py-1.5 text-xs bg-neutral-700 hover:bg-neutral-600 rounded transition-colors"
                  >
                    Browse
                  </button>
                </div>
              </div>

              {/* Move vs copy */}
              <label className="flex items-center gap-2 text-xs text-neutral-400 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={moveFiles}
                  onChange={(e) => setMoveFiles(e.target.checked)}
                  className="accent-blue-500"
                />
                Move files (remove from original location)
              </label>
            </>
          )}

          {/* Resize mode */}
          <div className="flex flex-col gap-1.5">
            <label className="text-[11px] text-neutral-500">Resize</label>
            <div className="flex gap-1.5">
              {(["none", "half", "fixed"] as const).map((m) => (
                <button
                  key={m}
                  onClick={() => setResizeMode(m)}
                  className={`text-[11px] px-2.5 py-1 rounded transition-colors ${
                    resizeMode === m
                      ? "bg-blue-600 text-white"
                      : "bg-neutral-800 text-neutral-400 hover:text-white"
                  }`}
                >
                  {m === "none" ? "No resize" : m === "half" ? "Half size" : "Fixed"}
                </button>
              ))}
            </div>
            {resizeMode === "fixed" && (
              <div className="flex items-center gap-2 mt-1">
                <input
                  type="number"
                  value={fixedW}
                  min={1}
                  onChange={(e) => setFixedW(Number(e.target.value))}
                  className="w-20 bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-xs text-neutral-200 text-center"
                />
                <span className="text-neutral-500 text-xs">×</span>
                <input
                  type="number"
                  value={fixedH}
                  min={1}
                  onChange={(e) => setFixedH(Number(e.target.value))}
                  className="w-20 bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-xs text-neutral-200 text-center"
                />
                <span className="text-[11px] text-neutral-600">px — orientation-aware, never upscale</span>
              </div>
            )}
            {resizeMode === "half" && (
              <p className="text-[11px] text-neutral-600">Each dimension will be halved.</p>
            )}
          </div>

          {/* Format & quality */}
          <div className="flex gap-4">
            <div className="flex flex-col gap-1">
              <label className="text-[11px] text-neutral-500">Format</label>
              <select
                value={targetFormat}
                onChange={(e) => setTargetFormat(e.target.value as "jpeg" | "webp")}
                className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-xs text-neutral-200 [color-scheme:dark]"
              >
                <option value="jpeg">JPEG</option>
                <option value="webp">WebP</option>
              </select>
            </div>
            <div className="flex flex-col gap-1 flex-1">
              <label className="text-[11px] text-neutral-500">Quality: {quality}%</label>
              <input
                type="range"
                min={1}
                max={100}
                value={quality}
                onChange={(e) => setQuality(Number(e.target.value))}
                className="accent-blue-500"
              />
            </div>
          </div>

          {/* Actions */}
          <div className="flex items-center gap-2 pt-1">
            <button
              onClick={handlePreview}
              disabled={outputMode === "folder" && !outputDir}
              className="px-3 py-1.5 text-xs bg-blue-600 hover:bg-blue-500 rounded text-white disabled:opacity-40 transition-colors"
            >
              Preview &amp; Confirm
            </button>
            <button
              onClick={() => setShowProcess(false)}
              className="px-3 py-1.5 text-xs bg-neutral-700 hover:bg-neutral-600 rounded transition-colors"
            >
              Cancel
            </button>
            {processMutation.isSuccess && (
              <span className="text-xs text-green-400">Job started — check Jobs page</span>
            )}
            {processMutation.isError && (
              <span className="text-xs text-red-400">{String(processMutation.error)}</span>
            )}
          </div>
        </div>
      )}

      {expanded && (
        <div className="p-3">
          <div className="flex items-center gap-1.5 mb-2">
            <span className="text-[11px] text-neutral-500">Sort:</span>
            {(Object.keys(SORT_LABELS) as SortBy[]).map((s) => (
              <button
                key={s}
                onClick={() => setSortBy(s)}
                className={`text-[11px] px-2 py-0.5 rounded transition-colors ${
                  sortBy === s
                    ? "bg-blue-600 text-white"
                    : "bg-neutral-800 text-neutral-400 hover:text-white"
                }`}
              >
                {SORT_LABELS[s]}
              </button>
            ))}
          </div>
          <div className="grid grid-cols-6 gap-1.5">
            {allImages.map((img) => (
              <ImageThumb
                key={img.id}
                image={img}
                groupId={group.id}
                onRemoved={handleImageRemoved}
                onTrashed={handleImageTrashed}
                onOpen={onOpenImage}
              />
            ))}
          </div>
          {hasMore && (
            <button
              onClick={() => setPage((p) => p + 1)}
              disabled={isFetching}
              className="mt-3 w-full py-1.5 text-xs bg-neutral-800 hover:bg-neutral-700 rounded disabled:opacity-40 transition-colors"
            >
              {isFetching ? "Loading…" : `Load more (${allImages.length} / ${total})`}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default function GalleryPage() {
  const [granularity, setGranularity] = useState<Granularity>("day");
  const [lightboxImage, setLightboxImage] = useState<Image | null>(null);
  const queryClient = useQueryClient();

  const { data: groups, isLoading } = useQuery({
    queryKey: ["groups", `date_${granularity}`],
    queryFn: () => getGroups(`date_${granularity}`),
  });

  const rebuild = useMutation({
    mutationFn: () => rebuildGroups("date"),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["groups"] }),
  });

  // Local list so removed groups disappear immediately without a full refetch
  const [removedGroupIds, setRemovedGroupIds] = useState<Set<number>>(new Set());

  const visibleGroups = groups?.filter((g) => !removedGroupIds.has(g.id));

  const handleGroupRemoved = (groupId: number) => {
    setRemovedGroupIds((prev) => new Set(prev).add(groupId));
  };

  const handleOpenImage = useCallback((image: Image) => {
    setLightboxImage(image);
  }, []);

  return (
    <div className="flex flex-col gap-4">
      {lightboxImage && (
        <Lightbox image={lightboxImage} onClose={() => setLightboxImage(null)} />
      )}
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">Gallery</h1>
        <div className="flex items-center gap-3">
          <div className="flex rounded-lg overflow-hidden border border-neutral-700 text-xs">
            {(["day", "month", "year"] as Granularity[]).map((g) => (
              <button
                key={g}
                onClick={() => setGranularity(g)}
                className={`px-3 py-1.5 capitalize transition-colors ${
                  granularity === g
                    ? "bg-blue-600 text-white"
                    : "bg-neutral-800 text-neutral-400 hover:text-white"
                }`}
              >
                {g}
              </button>
            ))}
          </div>
          <button
            onClick={() => rebuild.mutate()}
            disabled={rebuild.isPending}
            className="px-3 py-1.5 text-xs bg-neutral-700 hover:bg-neutral-600 rounded-lg disabled:opacity-40 transition-colors"
          >
            {rebuild.isPending ? "Rebuilding…" : "Rebuild Groups"}
          </button>
        </div>
      </div>

      {isLoading && <p className="text-sm text-neutral-500">Loading…</p>}
      {visibleGroups?.length === 0 && (
        <p className="text-sm text-neutral-500">
          No groups yet — scan a folder first, then click Rebuild Groups.
        </p>
      )}

      <div className="flex flex-col gap-2">
        {visibleGroups?.map((g) => (
          <GroupCard key={g.id} group={g} onRemoved={handleGroupRemoved} onOpenImage={handleOpenImage} />
        ))}
      </div>
    </div>
  );
}
