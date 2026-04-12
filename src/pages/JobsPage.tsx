import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getJobs, createJob, startJob, cancelJob, retryFailedImages, getImages } from "../api/commands";
import { useJobStore } from "../store/jobStore";
import type { ProcessingJob } from "../api/commands";

function JobItem({ job }: { job: ProcessingJob }) {
  const queryClient = useQueryClient();
  const progress = useJobStore((s) => s.activeJobProgress[job.id]);

  const displayed = progress ?? {
    processed: job.processedCount + job.failedCount,
    total: job.totalImages,
  };

  const pct = displayed.total > 0
    ? Math.round((displayed.processed / displayed.total) * 100)
    : 0;

  const cancel = useMutation({
    mutationFn: () => cancelJob(job.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["jobs"] }),
  });

  const retry = useMutation({
    mutationFn: () => retryFailedImages(job.id),
    onSuccess: ({ jobId }) => {
      queryClient.invalidateQueries({ queryKey: ["jobs"] });
      startJob(jobId);
    },
  });

  const statusColor: Record<string, string> = {
    queued: "text-neutral-400",
    running: "text-blue-400",
    done: "text-green-400",
    failed: "text-red-400",
    cancelled: "text-neutral-500",
  };

  return (
    <div className="bg-neutral-900 rounded-lg p-4 border border-neutral-800 flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium capitalize">{job.operation}</span>
          <span className={`text-xs capitalize ${statusColor[job.status] ?? "text-neutral-400"}`}>
            {job.status}
          </span>
        </div>
        <div className="flex gap-2">
          {job.status === "running" && (
            <button
              onClick={() => cancel.mutate()}
              className="text-xs text-neutral-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
          )}
          {job.status === "failed" && job.failedCount > 0 && (
            <button
              onClick={() => retry.mutate()}
              className="text-xs text-orange-400 hover:text-orange-300 transition-colors"
            >
              Retry {job.failedCount} failed
            </button>
          )}
        </div>
      </div>
      <div className="w-full h-1.5 bg-neutral-800 rounded-full overflow-hidden">
        <div
          className={`h-full transition-all ${
            job.status === "done" ? "bg-green-500" : job.status === "failed" ? "bg-red-500" : "bg-blue-500"
          }`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <p className="text-xs text-neutral-500">
        {displayed.processed} / {displayed.total} images
      </p>
    </div>
  );
}

export default function JobsPage() {
  const queryClient = useQueryClient();
  const [quality, setQuality] = useState(85);
  const [targetFormat, setTargetFormat] = useState<"jpeg" | "webp">("jpeg");
  const [outputMode, setOutputMode] = useState<"output_folder" | "in_place">("output_folder");
  const [selectedImageIds] = useState<number[]>([]);

  const { data: jobs } = useQuery({
    queryKey: ["jobs"],
    queryFn: () => getJobs({}),
    refetchInterval: 2000,
  });

  const { data: allImages } = useQuery({
    queryKey: ["images", null, 0],
    queryFn: () => getImages({ page: 0, pageSize: 200 }),
  });

  const createAndStart = useMutation({
    mutationFn: async () => {
      const imageIds = selectedImageIds.length > 0
        ? selectedImageIds
        : (allImages?.items.map((i) => i.id) ?? []);

      const { jobId } = await createJob({
        imageIds,
        operation: "compress",
        params: { quality, targetFormat },
        outputMode,
      });
      await startJob(jobId);
      return jobId;
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["jobs"] }),
  });

  return (
    <div className="flex flex-col gap-6 max-w-2xl">
      <h1 className="text-xl font-bold">Jobs</h1>

      <div className="bg-neutral-900 rounded-lg p-4 border border-neutral-800 flex flex-col gap-4">
        <h2 className="text-sm font-semibold text-neutral-200">New Batch Job</h2>

        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col gap-1">
            <label className="text-xs text-neutral-400">Format</label>
            <select
              value={targetFormat}
              onChange={(e) => setTargetFormat(e.target.value as "jpeg" | "webp")}
              className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1.5 text-sm"
            >
              <option value="jpeg">JPEG</option>
              <option value="webp">WebP</option>
            </select>
          </div>

          <div className="flex flex-col gap-1">
            <label className="text-xs text-neutral-400">Output</label>
            <select
              value={outputMode}
              onChange={(e) => setOutputMode(e.target.value as "output_folder" | "in_place")}
              className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1.5 text-sm"
            >
              <option value="output_folder">Output Folder</option>
              <option value="in_place">Overwrite In-Place</option>
            </select>
          </div>
        </div>

        <div className="flex flex-col gap-1">
          <label className="text-xs text-neutral-400">
            Quality: {quality}
          </label>
          <input
            type="range"
            min={1}
            max={100}
            value={quality}
            onChange={(e) => setQuality(Number(e.target.value))}
            className="w-full accent-blue-500"
          />
        </div>

        <div className="flex items-center justify-between">
          <p className="text-xs text-neutral-500">
            {selectedImageIds.length > 0
              ? `${selectedImageIds.length} selected images`
              : `All ${allImages?.total ?? 0} images`}
          </p>
          <button
            onClick={() => createAndStart.mutate()}
            disabled={createAndStart.isPending || (allImages?.total ?? 0) === 0}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded-lg text-sm font-medium transition-colors"
          >
            {createAndStart.isPending ? "Starting…" : "Start Job"}
          </button>
        </div>
      </div>

      <div className="flex flex-col gap-2">
        <h2 className="text-sm font-semibold text-neutral-400">Recent Jobs</h2>
        {jobs?.items.map((job) => <JobItem key={job.id} job={job} />)}
        {jobs?.items.length === 0 && (
          <p className="text-sm text-neutral-500">No jobs yet.</p>
        )}
      </div>
    </div>
  );
}
