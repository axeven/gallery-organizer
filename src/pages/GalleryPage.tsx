import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getGroups, rebuildGroups, getThumbnail, getImages } from "../api/commands";
import type { Group } from "../api/commands";

type Granularity = "day" | "month" | "year";

function ImageThumb({ imageId }: { imageId: number }) {
  const { data: src } = useQuery({
    queryKey: ["thumbnail", imageId],
    queryFn: () => getThumbnail(imageId),
    staleTime: Infinity,
  });

  return (
    <div className="aspect-square bg-neutral-800 rounded overflow-hidden">
      {src ? (
        <img
          src={`data:image/jpeg;base64,${src}`}
          className="w-full h-full object-cover"
          loading="lazy"
        />
      ) : (
        <div className="w-full h-full animate-pulse bg-neutral-700" />
      )}
    </div>
  );
}

function GroupCard({ group }: { group: Group }) {
  const [expanded, setExpanded] = useState(false);
  const { data } = useQuery({
    queryKey: ["images", group.id, 0],
    queryFn: () => getImages({ groupId: group.id, page: 0, pageSize: 20 }),
    enabled: expanded,
  });

  return (
    <div className="bg-neutral-900 rounded-lg overflow-hidden border border-neutral-800">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-4 py-3 hover:bg-neutral-800 transition-colors text-left"
      >
        <div>
          <span className="font-medium text-sm">{group.label}</span>
          <span className="ml-2 text-xs text-neutral-500">{group.imageCount} images</span>
        </div>
        <span className="text-neutral-500 text-xs">{expanded ? "▲" : "▼"}</span>
      </button>
      {expanded && (
        <div className="p-3 grid grid-cols-6 gap-1.5">
          {data?.items.map((img) => (
            <ImageThumb key={img.id} imageId={img.id} />
          ))}
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
      {groups?.length === 0 && (
        <p className="text-sm text-neutral-500">
          No groups yet — scan a folder first, then click Rebuild Groups.
        </p>
      )}

      <div className="flex flex-col gap-2">
        {groups?.map((g) => <GroupCard key={g.id} group={g} />)}
      </div>
    </div>
  );
}
