import { useEffect, useRef, useState } from "react";
import { invoke } from '@tauri-apps/api/core';
import { useMapContext } from "../../context/mapContext";
import { useVrcContext } from "../../context/VrcContext";

/**
 * Dropdown and sliders for editing VRC config node radii.
 * Uses the `set_node_radius` Tauri command to persist changes.
 */
export default function VrcConfigRadiusEditor() {
  const { vrcInfo } = useVrcContext();
  const { globalMap } = useMapContext();
  const [selectedConfigIdx, setSelectedConfigIdx] = useState<number>(0);
  const [radii, setRadii] = useState<number[]>([]);
  const [saving, setSaving] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [multiplier, setMultiplier] = useState<number>(1);
  const applyTimerRef = useRef<number | null>(null);
  const [baselineAvg, setBaselineAvg] = useState<number>(0);

  // Get configs from avatar
  const configs = vrcInfo?.avatar?.configs ?? [];
  const configNames = configs.map((c: any, i: number) => c?.meta?.map_name || `Config ${i + 1}`);
  const nodes = configs[selectedConfigIdx]?.nodes ?? [];

  // Hoisted helper to avoid TDZ when the component early-returns.
  function averageRadius(list: number[]) {
    if (!list.length) return 0;
    return list.reduce((a, b) => a + b, 0) / list.length;
  }

  // When switching configs or VRC info changes, recompute baseline from VRC data,
  // clear local overrides and reset multiplier. Baseline stays fixed while sliding
  // to avoid feedback loops.
  useEffect(() => {
    const base = averageRadius(nodes.map((n: any) => n.radius));
    setBaselineAvg(base);
    setRadii([]);
    setMultiplier(1);
  }, [selectedConfigIdx, vrcInfo]);

  const handleRadiusChange = async (nodeIdx: number, newRadius: number) => {
    setRadii(r => r.map((val, i) => (i === nodeIdx ? newRadius : val)));
    const node = nodes[nodeIdx];
    if (!node) return;
    setSaving(true);
    setError(null);
    try {
      await invoke("set_node_radius", {
        id: node.address,
        radius: newRadius,
      });
    } catch (e: any) {
      console.error("Failed to set radius", e);
      setError(e?.message || "Failed to set radius");
    } finally {
      setSaving(false);
    }
  };

  if (!configs.length) return null;

  const handleApplyAllDebounced = (tag: string, radius: number) => {
    // clear any existing timer
    if (applyTimerRef.current) {
      clearTimeout(applyTimerRef.current);
    }
    // debounce to reduce invoke spam while sliding
    applyTimerRef.current = window.setTimeout(async () => {
      setSaving(true);
      setError(null);
      try {
        await invoke("set_tags_radius", { tag, radius });
      } catch (e: any) {
        console.error("Failed to set tags radius", e);
        setError(e?.message || "Failed to set tags radius");
      } finally {
        setSaving(false);
      }
    }, 250);
  };

  const handleMultiplierChange = (m: number) => {
    setMultiplier(m);
    const meta = configs[selectedConfigIdx]?.meta ?? {};
    const tag = `${meta.map_author ?? ""}_${meta.map_name ?? ""}_${meta.map_version ?? ""}`;
    // Use fixed baseline derived from VRC config radii to avoid feedback
    const newRadius = Number((baselineAvg * m).toFixed(6));
    // Update UI immediately
    setRadii(Array(nodes.length).fill(newRadius) as number[]);
    // Apply to backend after debounce
    if (tag && isFinite(newRadius)) handleApplyAllDebounced(tag, newRadius);
  };

  return (
    <div className="absolute top-2 left-2 bg-black/70 text-white p-4 rounded shadow max-w-xs z-10">
      {/* Global multiplier based on average radius */}
      <div className="mb-2">
        <div className="mb-1 flex items-center justify-between text-xs">
          <span>Scale Entire Prefab</span>
          <span className="tabular-nums">×{multiplier.toFixed(2)}</span>
        </div>
        <input
          type="range"
          min={0.25}
          max={3}
          step={0.01}
          value={multiplier}
          onChange={(e) => handleMultiplierChange(parseFloat(e.target.value))}
          className="w-full"
        />
        <div className="mt-1 text-[10px] text-gray-300">
          baseline avg: {baselineAvg.toFixed(3)} · target: {(baselineAvg * multiplier).toFixed(3)}
        </div>
      </div>

      <div className="mb-2 font-bold flex items-center justify-between">
        <span>Edit Node Radii</span>
        {saving && <span className="text-xs animate-pulse">Saving…</span>}
      </div>
      <select
        className="mb-2 w-full text-black"
        value={selectedConfigIdx}
        onChange={(e) => setSelectedConfigIdx(Number(e.target.value))}
      >
        {configNames.map((name, i) => (
          <option value={i} key={i}>
            {name}
          </option>
        ))}
      </select>
      <div className="space-y-2 max-h-56 overflow-y-auto pr-1">
        {nodes.map((node: any, idx: number) => (
          <div key={node.address} className="flex items-center gap-2">
            <span className="truncate text-xs" title={node.address}>
              {node.address}
            </span>
            <input
              type="range"
              min={0.01}
              max={0.2}
              step={0.001}
              value={(
                (() => {
                  const id = node.address as string;
                  const globalR = globalMap.input_nodes?.[id]?.radius;
                  const base = typeof globalR === "number" ? globalR : node.radius;
                  return radii[idx] ?? base;
                })()
              )}
              onChange={(e) => handleRadiusChange(idx, parseFloat(e.target.value))}
              className="flex-1"
            />
            <span className="w-10 text-right tabular-nums text-xs">
              {(() => {
                const id = node.address as string;
                const globalR = globalMap.input_nodes?.[id]?.radius;
                const base = typeof globalR === "number" ? globalR : node.radius;
                const val = radii[idx] ?? base;
                return (typeof val === "number" ? val : 0).toFixed(3);
              })()}
            </span>
          </div>
        ))}
      </div>
      {error && <div className="mt-2 text-xs text-red-400">{error}</div>}
    </div>
  );
}
