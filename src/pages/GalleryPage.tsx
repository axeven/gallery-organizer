import { useState, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getGroups, rebuildGroups, getThumbnail, getImages, removeGroup, removeImageFromGroup } from "../api/commands";
import type { Group, Image } from "../api/commands";

type Granularity = "day" | "month" | "year";

function ImageThumb({
  image,
  groupId,
  onRemoved,
}: {
  image: Image;
  groupId: number;
  onRemoved: (imageId: number) => void;
}) {
  const queryClient = useQueryClient();
  const [confirmRemove, setConfirmRemove] = useState(false);

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

  const res =
    image.widthPx && image.heightPx
      ? `${image.widthPx}×${image.heightPx}`
      : null;

  return (
    <div className="relative aspect-square bg-neutral-800 rounded overflow-hidden group/thumb">
      {src ? (
        <img
          src={`data:image/jpeg;base64,${src}`}
          className="w-full h-full object-cover"
          loading="lazy"
        />
      ) : (
        <div className="w-full h-full animate-pulse bg-neutral-700" />
      )}
      {res && (
        <span className="absolute bottom-0 inset-x-0 text-center text-[9px] leading-tight py-0.5 bg-black/60 text-neutral-300 truncate">
          {res}
        </span>
      )}
      {/* Remove button — visible on hover */}
      {!confirmRemove && (
        <button
          onClick={(e) => { e.stopPropagation(); setConfirmRemove(true); }}
          className="absolute top-1 right-1 opacity-0 group-hover/thumb:opacity-100 transition-opacity bg-black/70 hover:bg-red-700 text-white rounded text-[10px] px-1 py-0.5 leading-none"
          title="Remove from group"
        >
          ✕
        </button>
      )}
      {confirmRemove && (
        <div className="absolute inset-0 bg-black/75 flex flex-col items-center justify-center gap-1 p-1">
          <span className="text-[10px] text-center text-white">Remove?</span>
          <div className="flex gap-1">
            <button
              onClick={(e) => { e.stopPropagation(); removeMutation.mutate(); }}
              disabled={removeMutation.isPending}
              className="text-[10px] px-1.5 py-0.5 bg-red-600 hover:bg-red-500 rounded text-white disabled:opacity-40"
            >
              Yes
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); setConfirmRemove(false); }}
              className="text-[10px] px-1.5 py-0.5 bg-neutral-600 hover:bg-neutral-500 rounded text-white"
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

function GroupCard({ group, onRemoved }: { group: Group; onRemoved: (groupId: number) => void }) {
  const [expanded, setExpanded] = useState(false);
  const [page, setPage] = useState(0);
  const [allImages, setAllImages] = useState<Image[]>([]);
  const [total, setTotal] = useState<number>(group.imageCount ?? 0);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const queryClient = useQueryClient();

  const { data: pageData, isFetching } = useQuery({
    queryKey: ["images", group.id, page],
    queryFn: () => getImages({ groupId: group.id, page, pageSize: PAGE_SIZE }),
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
    setTotal(pageData.total);
  }, [pageData]);

  const removeGroupMutation = useMutation({
    mutationFn: () => removeGroup(group.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["groups"] });
      onRemoved(group.id);
    },
  });

  const handleImageRemoved = (imageId: number) => {
    setAllImages((prev) => prev.filter((img) => img.id !== imageId));
    setTotal((t) => t - 1);
  };

  const hasMore = allImages.length < total;

  return (
    <div className="bg-neutral-900 rounded-lg overflow-hidden border border-neutral-800">
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
        {/* Group-level remove */}
        {!confirmRemove ? (
          <button
            onClick={() => setConfirmRemove(true)}
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
      {expanded && (
        <div className="p-3">
          <div className="grid grid-cols-6 gap-1.5">
            {allImages.map((img) => (
              <ImageThumb
                key={img.id}
                image={img}
                groupId={group.id}
                onRemoved={handleImageRemoved}
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

  return (
    <div className="flex flex-col gap-4">
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
          <GroupCard key={g.id} group={g} onRemoved={handleGroupRemoved} />
        ))}
      </div>
    </div>
  );
}
