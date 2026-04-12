import { useScanStore } from "../../store/scanStore";
import { useJobStore } from "../../store/jobStore";

export default function TopBar() {
  const { isScanning, scanned, total, phase } = useScanStore();
  const { activeJobProgress } = useJobStore();
  const activeJobCount = Object.keys(activeJobProgress).length;

  return (
    <header className="h-10 bg-neutral-900 border-b border-neutral-800 flex items-center px-4 gap-4">
      {isScanning && (
        <div className="flex items-center gap-2 text-xs text-neutral-400">
          <div className="w-2 h-2 rounded-full bg-blue-500 animate-pulse" />
          <span>
            Scanning ({phase})… {scanned}/{total || "?"}
          </span>
          <div className="w-24 h-1 bg-neutral-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-blue-500 transition-all"
              style={{ width: total ? `${(scanned / total) * 100}%` : "0%" }}
            />
          </div>
        </div>
      )}
      {activeJobCount > 0 && (
        <span className="ml-auto text-xs bg-orange-600 text-white px-2 py-0.5 rounded-full">
          {activeJobCount} job{activeJobCount > 1 ? "s" : ""} running
        </span>
      )}
    </header>
  );
}
