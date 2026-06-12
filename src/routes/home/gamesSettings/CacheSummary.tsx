import { FC } from "react";
import { VrcInfo, OscInfo, AddrInfo, SpectaOscType } from "../../../bindings";

interface OscSummaryProps {
  vrcInfo: VrcInfo;
}

function oscValueToString(v: SpectaOscType): string {
  if (v === "Nil") return "Nil";
  if (v === "Inf") return "Inf";

  const num = (n: number | null | undefined) =>
    n == null ? "null" : Number.isFinite(n) ? n.toFixed(4) : String(n);

  if ("Float" in v) return num(v.Float);
  if ("Double" in v) return num(v.Double);
  if ("Int" in v) return v.Int == null ? "null" : String(v.Int);
  if ("Long" in v) return v.Long == null ? "null" : String(v.Long);
  if ("Bool" in v) return String(v.Bool);
  if ("String" in v) return v.String == null ? "null" : `"${v.String}"`;
  if ("Char" in v) return v.Char == null ? "null" : `'${v.Char}'`;
  if ("Blob" in v) return v.Blob == null ? "blob[null]" : `blob[${v.Blob.length}]`;
  if ("Array" in v)
    return v.Array == null ? "[]" : `[${v.Array.map(oscValueToString).join(", ")}]`;
  if ("Color" in v)
    return v.Color == null
      ? "rgba(null)"
      : `rgba(${v.Color.r}, ${v.Color.g}, ${v.Color.b}, ${v.Color.a})`;
  if ("Time" in v)
    return v.Time == null ? "time(null)" : `${v.Time.seconds}.${v.Time.fractional}`;
  if ("Midi" in v)
    return v.Midi == null
      ? "midi(null)"
      : `midi(${v.Midi.status},${v.Midi.data1},${v.Midi.data2})`;

  return "?";
}

/** Describe what an OSC address is wired to. */
function addrToString(addr: AddrInfo): string {
  if ("Slot" in addr && addr.Slot) {
    const [key, kind] = addr.Slot;
    return `Slot · node #${key.node.idx}, slot ${key.slot_idx} · ${kind}`;
  }
  if ("Node" in addr && addr.Node) {
    const [key, kind] = addr.Node;
    return `Node · #${key.idx} · ${kind}`;
  }
  return "unknown";
}

/** Stable per-entry key suffix: distinguishes multiple targets on one address. */
function addrKey(addr: AddrInfo): string {
  if ("Slot" in addr && addr.Slot) {
    const [key, kind] = addr.Slot;
    return `S:${key.node.idx}:${key.slot_idx}:${kind}`;
  }
  if ("Node" in addr && addr.Node) {
    const [key, kind] = addr.Node;
    return `N:${key.idx}:${kind}`;
  }
  return "U";
}

const shortPath = (p: string) => p.replace(/^\/avatar\/parameters\//, "");

const OscSummary: FC<OscSummaryProps> = ({ vrcInfo }) => {
  const available = (vrcInfo.available ?? [])
    .slice()
    .sort((a, b) => a.full_path.localeCompare(b.full_path));

  const watched = (vrcInfo.watched ?? [])
    .slice()
    .sort(([a], [b]) => a.localeCompare(b));

  const valueOf = (osc: OscInfo) =>
    osc.value.length ? osc.value.map(oscValueToString).join(", ") : "(no value)";

  return (
    <div>
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">OSC</h3>
        <span
          className={`badge badge-sm ${vrcInfo.is_connected ? "badge-success" : "badge-ghost"}`}
        >
          {vrcInfo.is_connected ? "connected" : "disconnected"}
        </span>
      </div>

      <div className="mt-2 text-sm md:text-base space-y-1">
        <p>
          <span className="font-medium">Ports:</span>{" "}
          in {vrcInfo.in_port ?? "—"} / out {vrcInfo.out_port ?? "—"}
        </p>
        <p>
          <span className="font-medium">Velocity:</span>{" "}
          ratio {vrcInfo.velocity_ratio.toFixed(2)}, mult {vrcInfo.velocity_mult.toFixed(2)}
        </p>

        {vrcInfo.avatar && (
          <p className="break-words">
            <span className="font-medium">Avatar:</span>{" "}
            <span className="font-mono text-xs">{vrcInfo.avatar.id}</span>
            {vrcInfo.avatar.prefab_names.length > 0 && (
              <span className="opacity-60">
                {" "}({vrcInfo.avatar.prefab_names.join(", ")})
              </span>
            )}
          </p>
        )}

        {/* Watched: the live address → target mapping */}
        <div className="collapse collapse-arrow border border-base-300 bg-base-100 rounded-box mt-1">
          <input type="checkbox" className="peer" />
          <div className="collapse-title font-medium">
            Watched Parameters: {watched.length}
          </div>
          <div className="collapse-content max-h-40 overflow-y-auto">
            <ul className="list-disc ml-4 mt-1 space-y-1 text-xs md:text-sm pr-2">
              {watched.map(([addr, info], _) => (
                <li key={`${addr}#${addrKey(info)}`} className="break-words">
                  <span className="font-mono">{shortPath(addr)}</span>
                  {" → "}
                  <span className="opacity-70">{addrToString(info)}</span>
                </li>
              ))}
            </ul>
          </div>
        </div>

        {/* Available: everything VRC advertises via OSCQuery */}
        <div className="collapse collapse-arrow border border-base-300 bg-base-100 rounded-box mt-1">
          <input type="checkbox" className="peer" />
          <div className="collapse-title font-medium">
            Available Parameters: {available.length}
          </div>
          <div className="collapse-content max-h-40 overflow-y-auto">
            <ul className="list-disc ml-4 mt-1 space-y-1 text-xs md:text-sm pr-2">
              {available.map((osc) => (
                <li key={osc.full_path} className="break-words" title={osc.description ?? ""}>
                  <span className="font-mono">{shortPath(osc.full_path)}</span>{" = "}
                  <span className="font-semibold">{valueOf(osc)}</span>{" "}
                  <span className="opacity-60">({osc.access})</span>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
};

export default OscSummary;