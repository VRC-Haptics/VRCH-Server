import { useMemo, useState } from "react";
import { useVrcContext } from "../../context/VrcContext";

export type NodeFilter =
  | { mode: "all" }
  | { mode: "prefab"; tag: string | null }
  | { mode: "tags"; tags: string[]; ignoreCase: boolean };

interface Props {
  filter: NodeFilter;
  onChange: (f: NodeFilter) => void;
}

function fmtTag(meta: any) {
  const a = meta?.map_author ?? "";
  const n = meta?.map_name ?? "";
  const v = meta?.map_version ?? "";
  return `${a}_${n}_${v}`;
}

export default function NodeFilterOverlay({ filter, onChange }: Props) {
  const { vrcInfo } = useVrcContext();
  const [tagInput, setTagInput] = useState<string>(
    filter.mode === "tags" ? filter.tags.join(", ") : ""
  );

  const prefabOptions = useMemo(() => {
    const configs = vrcInfo?.avatar?.configs ?? [];
    return configs.map((c: any) => ({
      label: c?.meta?.map_name || fmtTag(c?.meta),
      value: fmtTag(c?.meta),
    }));
  }, [vrcInfo]);

  const handleModeChange = (mode: NodeFilter["mode"]) => {
    if (mode === "all") onChange({ mode: "all" });
    else if (mode === "prefab") onChange({ mode: "prefab", tag: prefabOptions[0]?.value ?? null });
    else onChange({ mode: "tags", tags: parseTags(tagInput), ignoreCase: filter.mode === "tags" ? filter.ignoreCase : true });
  };

  const parseTags = (s: string) =>
    s
      .split(",")
      .map((t) => t.trim())
      .filter((t) => t.length > 0);

  const handlePrefabChange = (val: string) => {
    onChange({ mode: "prefab", tag: val || null });
  };

  const handleTagsChange = (val: string) => {
    setTagInput(val);
    onChange({ mode: "tags", tags: parseTags(val), ignoreCase: filter.mode === "tags" ? filter.ignoreCase : true });
  };

  return (
    <div className="absolute top-2 right-2 bg-black/70 text-white p-4 rounded shadow max-w-xs z-10 space-y-2">
      <div className="font-bold">Node Visibility</div>
      <div className="flex gap-2 text-xs">
        <button
          className={`px-2 py-1 rounded ${filter.mode === "all" ? "bg-white/20" : "bg-white/10"}`}
          onClick={() => handleModeChange("all")}
        >
          All
        </button>
        <button
          className={`px-2 py-1 rounded ${filter.mode === "prefab" ? "bg-white/20" : "bg-white/10"}`}
          onClick={() => handleModeChange("prefab")}
        >
          VRC Prefab
        </button>
        <button
          className={`px-2 py-1 rounded ${filter.mode === "tags" ? "bg-white/20" : "bg-white/10"}`}
          onClick={() => handleModeChange("tags")}
        >
          Tags
        </button>
      </div>

      {filter.mode === "prefab" && (
        <label className="block text-xs">
          <span className="block mb-1">Prefab</span>
          <select
            className="w-full text-black"
            value={filter.tag ?? ""}
            onChange={(e) => handlePrefabChange(e.target.value)}
          >
            {prefabOptions.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
      )}

      {filter.mode === "tags" && (
        <div className="text-xs space-y-1">
          <label className="block">
            <span className="block mb-1">Tags (comma‑separated)</span>
            <input
              type="text"
              className="w-full text-black px-2 py-1 rounded"
              placeholder="Torso, Front, Head"
              value={tagInput}
              onChange={(e) => handleTagsChange(e.target.value)}
            />
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={filter.mode === "tags" ? filter.ignoreCase : true}
              onChange={(e) => onChange({ mode: "tags", tags: parseTags(tagInput), ignoreCase: e.target.checked })}
            />
            <span>Ignore case</span>
          </label>
          <span className="opacity-70 text-[10px] block">Partial match · Matches any tag</span>
        </div>
      )}
    </div>
  );
}
