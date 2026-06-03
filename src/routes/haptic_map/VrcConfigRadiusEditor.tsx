import { useEffect, useMemo, useRef, useState } from "react";
import { commands, type ConfNode, type NodeKeyDef, type AddrInfo } from "../../bindings";
import { useMapContext } from "../../context/mapContext";
import { useVrcContext } from "../../context/VrcContext";

const stripPrefix = (p: string) => p.replace(/^\/avatar\/parameters\//, "");

/** Pull the owning NodeKey out of a watched AddrInfo entry. */
function addrNodeKey(info: AddrInfo): NodeKeyDef | null {
  if ("Slot" in info && info.Slot) return info.Slot[0].node; // SlotKey.node
  if ("Node" in info && info.Node) return info.Node[0];
  return null;
}

const keyId = (k: NodeKeyDef) => `${k.idx}:${k.version}`;

/**
 * Dropdown and sliders for editing VRC config node radii.
 * Resolves OSC addresses → NodeKeys via vrcInfo.watched, then persists with
 * the `set_nodes_radius` command.
 */
export default function VrcConfigRadiusEditor() {
  const { vrcInfo } = useVrcContext();
  const { globalMap } = useMapContext();

  const [selectedConfigIdx, setSelectedConfigIdx] = useState(0);
  const [radii, setRadii] = useState<number[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [multiplier, setMultiplier] = useState(1);
  const [baselineAvg, setBaselineAvg] = useState(0);
  const applyTimerRef = useRef<number | null>(null);

  const configs = vrcInfo?.avatar?.configs ?? [];
  const confNodes: ConfNode[] = configs[selectedConfigIdx]?.nodes ?? [];

  // address (vrcPrefix + address) → NodeKey, rebuilt only when watched changes.
  const addrToKey = useMemo(() => {
    const m = new Map<string, NodeKeyDef>();
    for (const [addr, info] of vrcInfo?.watched ?? []) {
      const k = addrNodeKey(info);
      if (k) m.set(addr, k);
    }
    return m;
  }, [vrcInfo?.watched]);

  const confNodeKeys = (node: ConfNode): NodeKeyDef[] => {
    const out: NodeKeyDef[] = [];
    const seen = new Set<string>();
    for (const input of node.inputs) {
      const k = addrToKey.get(input.vrcPrefix + input.address);
      if (!k || seen.has(keyId(k))) continue;
      seen.add(keyId(k));
      out.push(k);
    }
    return out;
  };

  // every resolved key in the selected config, deduped — for the global scale.
  const allConfigKeys = useMemo(() => {
    const out: NodeKeyDef[] = [];
    const seen = new Set<string>();
    for (const node of confNodes) {
      for (const input of node.inputs) {
        const k = addrToKey.get(input.vrcPrefix + input.address);
        if (!k || seen.has(keyId(k))) continue;
        seen.add(keyId(k));
        out.push(k);
      }
    }
    return out;
  }, [configs, selectedConfigIdx, addrToKey]);

  const averageRadius = (list: number[]) =>
    list.length ? list.reduce((a, b) => a + b, 0) / list.length : 0;

  // On config switch: reset baseline (from config-defined radii), clear overrides.
  useEffect(() => {
    setBaselineAvg(averageRadius(confNodes.map((n) => n.radius)));
    setRadii([]);
    setMultiplier(1);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedConfigIdx]);

  // On VRC poll: refresh baseline only if it actually moved; keep overrides.
  useEffect(() => {
    const base = averageRadius(confNodes.map((n) => n.radius));
    if (Math.abs(base - baselineAvg) > 1e-6) setBaselineAvg(base);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [vrcInfo]);

  if (!configs.length) return null;

  // Live radius for a node from the running map, by slotmap index.
  const liveRadius = (key: NodeKeyDef | undefined): number | undefined => {
    if (!key) return undefined;
    const slot: any = globalMap?.input_nodes?.nodes?.[key.idx];
    if (slot == null) return undefined;
    const node = "location" in slot ? slot : slot.value;
    return node?.radius ?? undefined;
  };

  const currentRadius = (node: ConfNode, idx: number): number => {
    const live = liveRadius(confNodeKeys(node)[0]);
    const base = typeof live === "number" ? live : node.radius;
    return radii[idx] ?? base;
  };

  const handleRadiusChange = async (idx: number, newRadius: number) => {
    setRadii((r) => {
      const next = r.slice();
      next[idx] = newRadius;
      return next;
    });
    const keys = confNodeKeys(confNodes[idx]);
    if (!keys.length) {
      setError("No live node mapped for this address yet");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const res = await commands.setNodesRadius(keys, newRadius);
      if (res.status === "error") setError("Failed to set radius");
    } catch (e: any) {
      setError(e?.message ?? "Failed to set radius");
    } finally {
      setSaving(false);
    }
  };

  const applyAllDebounced = (radius: number) => {
    if (applyTimerRef.current) clearTimeout(applyTimerRef.current);
    applyTimerRef.current = window.setTimeout(async () => {
      setSaving(true);
      setError(null);
      try {
        const res = await commands.setNodesRadius(allConfigKeys, radius);
        if (res.status === "error") setError("Failed to scale prefab");
      } catch (e: any) {
        setError(e?.message ?? "Failed to scale prefab");
      } finally {
        setSaving(false);
      }
    }, 25);
  };

  const handleMultiplierChange = (m: number) => {
    setMultiplier(m);
    const newRadius = Number((baselineAvg * m).toFixed(6));
    setRadii(Array(confNodes.length).fill(newRadius));
    if (allConfigKeys.length && isFinite(newRadius)) applyAllDebounced(newRadius);
  };

  const configNames = configs.map(
    (c, i) => c?.identification?.mapName || `Config ${i + 1}`
  );

  return (
    <div className="absolute top-2 left-2 bg-black/70 text-white p-4 rounded shadow max-w-xs z-10">
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
        {confNodes.map((node, idx) => {
          const primary = node.inputs[0]
            ? node.inputs[0].vrcPrefix + node.inputs[0].address
            : node.parentBone;
          const allAddrs = node.inputs
            .map((i) => i.vrcPrefix + i.address)
            .join("\n");
          const val = currentRadius(node, idx);
          const mapped = confNodeKeys(node).length > 0;
          return (
            <div key={`${selectedConfigIdx}-${idx}`} className="flex items-center gap-2">
              <span
                className={`truncate text-xs ${mapped ? "" : "opacity-40"}`}
                title={allAddrs || node.parentBone}
              >
                {stripPrefix(primary)}
              </span>
              <input
                type="range"
                min={0.01}
                max={0.5}
                step={0.001}
                value={val}
                disabled={!mapped}
                onChange={(e) => handleRadiusChange(idx, parseFloat(e.target.value))}
                className="flex-1"
              />
              <span className="w-10 text-right tabular-nums text-xs">
                {val.toFixed(3)}
              </span>
            </div>
          );
        })}
      </div>

      {error && <div className="mt-2 text-xs text-red-400">{error}</div>}
    </div>
  );
}