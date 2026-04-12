import { useState } from "react";
import { openFolderDialog, scanFolder, cancelScan } from "../api/commands";
import { useScanStore } from "../store/scanStore";

export default function ScanPage() {
  const [folderPath, setFolderPath] = useState("");
  const [recursive, setRecursive] = useState(true);
  const { isScanning, phase, scanned, total, currentPath, resetScan } = useScanStore();

  const handlePickFolder = async () => {
    const path = await openFolderDialog();
    if (path) setFolderPath(path);
  };

  const handleScan = async () => {
    if (!folderPath) return;
    resetScan();
    useScanStore.getState().setScanProgress({
      phase: "walking",
      scanned: 0,
      total: 0,
      currentPath: folderPath,
    });
    try {
      await scanFolder(folderPath, recursive);
    } catch (e) {
      console.error("scan_folder error:", e);
      resetScan();
    }
  };

  const handleCancel = async () => {
    await cancelScan();
    resetScan();
  };

  const progress = total > 0 ? (scanned / total) * 100 : 0;
  const isDone = phase === "done";

  return (
    <div className="max-w-xl mx-auto mt-12 flex flex-col gap-6">
      <h1 className="text-2xl font-bold">Scan Folder</h1>

      <div className="flex gap-2">
        <input
          type="text"
          value={folderPath}
          readOnly
          placeholder="Select a folder to scan…"
          className="flex-1 bg-neutral-800 border border-neutral-700 rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-neutral-500 cursor-pointer"
          onClick={handlePickFolder}
        />
        <button
          onClick={handlePickFolder}
          className="px-4 py-2 bg-neutral-700 hover:bg-neutral-600 rounded-lg text-sm transition-colors"
        >
          Browse
        </button>
      </div>

      <label className="flex items-center gap-3 text-sm text-neutral-300 cursor-pointer">
        <input
          type="checkbox"
          checked={recursive}
          onChange={(e) => setRecursive(e.target.checked)}
          className="w-4 h-4 accent-blue-500"
        />
        Scan subfolders recursively
      </label>

      <div className="flex gap-3">
        <button
          onClick={handleScan}
          disabled={!folderPath || isScanning}
          className="px-6 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed rounded-lg text-sm font-medium transition-colors"
        >
          {isScanning ? "Scanning…" : "Start Scan"}
        </button>
        {isScanning && (
          <button
            onClick={handleCancel}
            className="px-4 py-2 bg-neutral-700 hover:bg-red-600 rounded-lg text-sm transition-colors"
          >
            Cancel
          </button>
        )}
      </div>

      {(isScanning || isDone) && (
        <div className="flex flex-col gap-2">
          <div className="flex justify-between text-xs text-neutral-400">
            <span className="capitalize">{phase}…</span>
            <span>{scanned} / {total || "?"}</span>
          </div>
          <div className="w-full h-2 bg-neutral-800 rounded-full overflow-hidden">
            <div
              className="h-full bg-blue-500 transition-all duration-150"
              style={{ width: `${progress}%` }}
            />
          </div>
          {currentPath && (
            <p className="text-xs text-neutral-500 truncate">{currentPath}</p>
          )}
          {isDone && (
            <p className="text-sm text-green-400 font-medium">
              Done — {scanned} images scanned
            </p>
          )}
        </div>
      )}
    </div>
  );
}
