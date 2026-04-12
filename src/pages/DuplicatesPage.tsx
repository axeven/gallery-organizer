import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getDuplicateClusters, rebuildGroups, setKeeper, dismissCluster, getThumbnail } from "../api/commands";
import type { DuplicateCluster, Image } from "../api/commands";
import { useQuery as useThumbQuery } from "@tanstack/react-query";

function Thumb({ image, isKeeper, onSetKeeper }: {
  image: Image;
  isKeeper: boolean;
  onSetKeeper: () => void;
}) {
  const { data: src } = useThumbQuery({
    queryKey: ["thumbnail", image.id],
    queryFn: () => getThumbnail(image.id),
    staleTime: Infinity,
  });

  const sizeKb = Math.round(image.fileSizeBytes / 1024);
  const dims = image.widthPx && image.heightPx ? `${image.widthPx}×${image.heightPx}` : "?";

  return (
    <div
      className={`flex flex-col gap-1 cursor-pointer rounded-lg overflow-hidden border-2 transition-colors ${
        isKeeper ? "border-green-500" : "border-transparent hover:border-neutral-600"
      }`}
      onClick={onSetKeeper}
    >
      <div className="aspect-square bg-neutral-800 rounded overflow-hidden">
        {src ? (
          <img src={`data:image/jpeg;base64,${src}`} className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full animate-pulse bg-neutral-700" />
        )}
      </div>
      <div className="px-1 pb-1 text-xs text-neutral-400 space-y-0.5">
        <p className="truncate">{image.fileName}</p>
        <p>{dims} · {sizeKb}KB</p>
        {isKeeper && <p className="text-green-400 font-medium">Keep</p>}
      </div>
    </div>
  );
}

function ClusterCard({ cluster }: { cluster: DuplicateCluster }) {
  const queryClient = useQueryClient();

  const keeperMutation = useMutation({
    mutationFn: ({ imageId }: { imageId: number }) =>
      setKeeper(cluster.clusterId, imageId),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["duplicate-clusters"] }),
  });

  const dismissMutation = useMutation({
    mutationFn: () => dismissCluster(cluster.clusterId),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["duplicate-clusters"] }),
  });

  return (
    <div className="bg-neutral-900 rounded-lg p-4 border border-neutral-800">
      <div className="flex items-center justify-between mb-3">
        <span className="text-sm text-neutral-400">
          {cluster.images.length} duplicates
        </span>
        <button
          onClick={() => dismissMutation.mutate()}
          className="text-xs text-neutral-500 hover:text-white transition-colors"
        >
          Dismiss
        </button>
      </div>
      <div className="grid grid-cols-4 gap-2">
        {cluster.images.map((img) => (
          <Thumb
            key={img.id}
            image={img}
            isKeeper={img.id === cluster.suggestedKeeperId}
            onSetKeeper={() => keeperMutation.mutate({ imageId: img.id })}
          />
        ))}
      </div>
    </div>
  );
}

export default function DuplicatesPage() {
  const queryClient = useQueryClient();

  const { data: clusters, isLoading } = useQuery({
    queryKey: ["duplicate-clusters"],
    queryFn: getDuplicateClusters,
  });

  const rebuild = useMutation({
    mutationFn: () => rebuildGroups("duplicates"),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["duplicate-clusters"] }),
  });

  const totalWastedKb = clusters?.reduce((acc, c) => {
    const sorted = [...c.images].sort((a, b) => b.fileSizeBytes - a.fileSizeBytes);
    return acc + sorted.slice(1).reduce((s, img) => s + img.fileSizeBytes, 0);
  }, 0) ?? 0;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold">Duplicates</h1>
          {clusters && clusters.length > 0 && (
            <p className="text-xs text-neutral-400 mt-0.5">
              {clusters.length} clusters · ~{Math.round(totalWastedKb / 1024)} MB wasted
            </p>
          )}
        </div>
        <button
          onClick={() => rebuild.mutate()}
          disabled={rebuild.isPending}
          className="px-3 py-1.5 text-xs bg-neutral-700 hover:bg-neutral-600 rounded-lg disabled:opacity-40 transition-colors"
        >
          {rebuild.isPending ? "Scanning…" : "Find Duplicates"}
        </button>
      </div>

      {isLoading && <p className="text-sm text-neutral-500">Loading…</p>}
      {clusters?.length === 0 && (
        <p className="text-sm text-neutral-500">
          No duplicates found. Click "Find Duplicates" after scanning.
        </p>
      )}

      <div className="flex flex-col gap-3">
        {clusters?.map((c) => <ClusterCard key={c.clusterId} cluster={c} />)}
      </div>
    </div>
  );
}
