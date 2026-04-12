import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getSettings, updateSettings, openFolderDialog } from "../api/commands";
import type { AppSettings } from "../api/commands";
import { useState, useEffect } from "react";

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
  });

  const [draft, setDraft] = useState<Partial<AppSettings>>({});

  useEffect(() => {
    if (settings) setDraft(settings);
  }, [settings]);

  const save = useMutation({
    mutationFn: (s: Partial<AppSettings>) => updateSettings(s),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["settings"] }),
  });

  const merged = { ...settings, ...draft } as AppSettings;

  const handleOutputDir = async () => {
    const path = await openFolderDialog();
    if (path) setDraft((d) => ({ ...d, outputDir: path }));
  };

  return (
    <div className="max-w-lg flex flex-col gap-6">
      <h1 className="text-xl font-bold">Settings</h1>

      <section className="bg-neutral-900 rounded-lg p-4 border border-neutral-800 flex flex-col gap-4">
        <h2 className="text-sm font-semibold">File Output</h2>

        <div className="flex flex-col gap-1">
          <label className="text-xs text-neutral-400">Output Mode</label>
          <select
            value={merged.outputMode ?? "output_folder"}
            onChange={(e) => setDraft((d) => ({ ...d, outputMode: e.target.value as AppSettings["outputMode"] }))}
            className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1.5 text-sm"
          >
            <option value="output_folder">Save to output folder</option>
            <option value="in_place">Overwrite originals in-place</option>
          </select>
        </div>

        {merged.outputMode === "output_folder" && (
          <div className="flex gap-2">
            <input
              type="text"
              value={merged.outputDir ?? ""}
              readOnly
              placeholder="Pick output directory…"
              className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-2 py-1.5 text-sm text-neutral-300 placeholder-neutral-600"
            />
            <button
              onClick={handleOutputDir}
              className="px-3 py-1.5 bg-neutral-700 hover:bg-neutral-600 rounded text-sm transition-colors"
            >
              Browse
            </button>
          </div>
        )}
      </section>

      <section className="bg-neutral-900 rounded-lg p-4 border border-neutral-800 flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Processing Defaults</h2>

        <div className="flex flex-col gap-1">
          <label className="text-xs text-neutral-400">Default Format</label>
          <select
            value={merged.defaultFormat ?? "jpeg"}
            onChange={(e) => setDraft((d) => ({ ...d, defaultFormat: e.target.value as AppSettings["defaultFormat"] }))}
            className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1.5 text-sm"
          >
            <option value="jpeg">JPEG</option>
            <option value="webp">WebP</option>
          </select>
        </div>

        <div className="flex flex-col gap-1">
          <label className="text-xs text-neutral-400">
            Default Quality: {merged.defaultQuality ?? 85}
          </label>
          <input
            type="range"
            min={1}
            max={100}
            value={merged.defaultQuality ?? 85}
            onChange={(e) => setDraft((d) => ({ ...d, defaultQuality: Number(e.target.value) }))}
            className="accent-blue-500"
          />
        </div>
      </section>

      <section className="bg-neutral-900 rounded-lg p-4 border border-neutral-800 flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Grouping</h2>

        <div className="flex flex-col gap-1">
          <label className="text-xs text-neutral-400">Date Group Granularity</label>
          <select
            value={merged.dateGroupGranularity ?? "day"}
            onChange={(e) => setDraft((d) => ({ ...d, dateGroupGranularity: e.target.value as AppSettings["dateGroupGranularity"] }))}
            className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1.5 text-sm"
          >
            <option value="day">By Day</option>
            <option value="month">By Month</option>
            <option value="year">By Year</option>
          </select>
        </div>

        <div className="flex flex-col gap-1">
          <label className="text-xs text-neutral-400">
            Duplicate Hash Distance: {merged.duplicateHashDistance ?? 8}
          </label>
          <input
            type="range"
            min={1}
            max={20}
            value={merged.duplicateHashDistance ?? 8}
            onChange={(e) => setDraft((d) => ({ ...d, duplicateHashDistance: Number(e.target.value) }))}
            className="accent-blue-500"
          />
          <p className="text-xs text-neutral-600">
            Lower = stricter (fewer false positives). Higher = looser (catches more edits).
          </p>
        </div>
      </section>

      <button
        onClick={() => save.mutate(draft)}
        disabled={save.isPending}
        className="self-start px-6 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded-lg text-sm font-medium transition-colors"
      >
        {save.isPending ? "Saving…" : "Save Settings"}
      </button>
    </div>
  );
}
