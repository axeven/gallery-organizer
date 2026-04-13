import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getDuplicateClusters, rebuildGroups, setKeeper, dismissCluster, getThumbnail, trashImage } from "../api/commands";
import type { DuplicateCluster, Image } from "../api/commands";

function Thumb({ image, isKeeper, onSetKeeper, onTrashed }: {
  image: Image;
  isKeeper: boolean;
  onSetKeeper: () => void;
  onTrashed: (imageId: number) => void;
}) {
  const queryClient = useQueryClient();
  const [confirmTrash, setConfirmTrash] = useState(false);

  const { data: src } = useQuery({
    queryKey: ["thumbnail", image.id],
    queryFn: () => getThumbnail(image.id),
    staleTime: Infinity,
  });

  const trashMutation = useMutation({
    mutationFn: () => trashImage(image.id),
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: ["thumbnail", image.id] });
      onTrashed(image.id);
    },
  });

  const res = image.widthPx && image.heightPx ? `${image.widthPx}×${image.heightPx}` : null;
  const sizeMb = (image.fileSizeBytes / 1024 / 1024).toFixed(1);

  return (
    <div
      className={`flex flex-col gap-1 rounded-lg overflow-hidden border-2 transition-colors ${
        isKeeper ? "border-green-500" : "border-transparent hover:border-neutral-600"
      }`}
    >
      {/* Thumbnail with hover actions */}
      <div
        className="relative aspect-square bg-neutral-800 rounded overflow-hidden group/thumb cursor-pointer"
        onClick={() => { if (!confirmTrash) onSetKeeper(); }}
      >
        {src ? (
          <img src={`data:image/jpeg;base64,${src}`} className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full animate-pulse bg-neutral-700" />
        )}

        {/* Resolution badge */}
        {res && (
          <span className="absolute bottom-0 inset-x-0 text-center text-[9px] leading-tight py-0.5 bg-black/60 text-neutral-300 truncate">
            {res}
          </span>
        )}

        {/* Trash button — visible on hover */}
        {!confirmTrash && (
          <div className="absolute top-1 right-1 opacity-0 group-hover/thumb:opacity-100 transition-opacity">
            <button
              onClick={(e) => { e.stopPropagation(); setConfirmTrash(true); }}
              className="bg-black/70 hover:bg-red-700 text-white rounded text-[10px] px-1 py-0.5 leading-none"
              title="Move to trash"
            >
              🗑
            </button>
          </div>
        )}

        {/* Trash confirmation overlay */}
        {confirmTrash && (
          <div className="absolute inset-0 bg-black/80 flex flex-col items-center justify-center gap-1.5 p-1">
            <span className="text-[10px] text-center text-white leading-tight">Move to trash?</span>
            <div className="flex gap-1">
              <button
                onClick={(e) => { e.stopPropagation(); trashMutation.mutate(); }}
                disabled={trashMutation.isPending}
                className="text-[10px] px-1.5 py-0.5 bg-red-600 hover:bg-red-500 rounded text-white disabled:opacity-40"
              >
                Yes
              </button>
              <button
                onClick={(e) => { e.stopPropagation(); setConfirmTrash(false); }}
                className="text-[10px] px-1.5 py-0.5 bg-neutral-700 hover:bg-neutral-600 rounded text-white"
              >
                No
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Info bar */}
      <div className="px-1 pb-1 text-xs text-neutral-400 space-y-0.5">
        <p className="truncate" title={image.fileName}>{image.fileName}</p>
        <p className="text-neutral-500">{sizeMb} MB</p>
        {isKeeper && <p className="text-green-400 font-medium">Keep</p>}
      </div>
    </div>
  );
}

function ClusterCard({ cluster }: { cluster: DuplicateCluster }) {
  const queryClient = useQueryClient();
  const [images, setImages] = useState<Image[]>(cluster.images);
  const [keeperId, setKeeperId] = useState<number | null>(cluster.suggestedKeeperId);

  const keeperMutation = useMutation({
    mutationFn: ({ imageId }: { imageId: number }) =>
      setKeeper(cluster.clusterId, imageId),
    onSuccess: (_, { imageId }) => setKeeperId(imageId),
  });

  const dismissMutation = useMutation({
    mutationFn: () => dismissCluster(cluster.clusterId),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["duplicate-clusters"] }),
  });

  const handleTrashed = (imageId: number) => {
    const next = images.filter((img) => img.id !== imageId);
    setImages(next);
    if (keeperId === imageId) setKeeperId(null);
    // If only one image left the cluster is no longer a duplicate — dismiss it
    if (next.length < 2) {
      dismissMutation.mutate();
    }
  };

  if (images.length < 2) return null;

  return (
    <div className="bg-neutral-900 rounded-lg p-4 border border-neutral-800">
      <div className="flex items-center justify-between mb-3">
        <span className="text-sm text-neutral-400">
          {images.length} duplicates
        </span>
        <button
          onClick={() => dismissMutation.mutate()}
          disabled={dismissMutation.isPending}
          className="text-xs text-neutral-500 hover:text-white transition-colors disabled:opacity-40"
        >
          Dismiss
        </button>
      </div>
      <div className="grid grid-cols-4 gap-2">
        {images.map((img) => (
          <Thumb
            key={img.id}
            image={img}
            isKeeper={img.id === keeperId}
            onSetKeeper={() => keeperMutation.mutate({ imageId: img.id })}
            onTrashed={handleTrashed}
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
