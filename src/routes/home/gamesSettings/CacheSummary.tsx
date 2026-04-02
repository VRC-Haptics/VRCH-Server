import { FC } from "react";
import { VrcInfo, SpectaOscType, CacheNode } from "../../../bindings";

interface OscSummaryProps {
  vrcInfo: VrcInfo;
}

type TimestampPair = {
  duration_since_epoch: number;
  duration_since_unix_epoch: number;
};

/** Pull a display string from a SpectaOscType variant */
function oscValueToString(v: SpectaOscType): string {
  if (v === "Nil") return "Nil";
  if (v === "Inf") return "Inf";
  if ("Float" in v) return (v.Float ?? 0).toFixed(4);
  if ("Int" in v) return String(v.Int);
  if ("Bool" in v) return String(v.Bool);
  if ("String" in v) return `"${v.String}"`;
  if ("Double" in v) return (v.Double ?? 0).toFixed(4);
  if ("Long" in v) return String(v.Long);
  if ("Char" in v) return `'${v.Char}'`;
  return "?";
}

const OscSummary: FC<OscSummaryProps> = ({ vrcInfo }) => {
  const availableCount = vrcInfo.available.length;

  const tsPairToMs = (t: TimestampPair) => t.duration_since_unix_epoch * 1000;

  const cached = vrcInfo.cached.map(([path, node]: [string, CacheNode]) => {
    const entry = node.values[0]; // most recent ring-buffer entry
    let ageMs = Number.POSITIVE_INFINITY;
    let displayValue = "- -";

    if (entry) {
      const [oscVal, ts] = entry;
      displayValue = oscValueToString(oscVal);
      ageMs = Date.now() - tsPairToMs(ts);
    }

    return { path, displayValue, ageMs };
  });

  cached.sort((a, b) => a.path.localeCompare(b.path));

  return (
    <div>
      <h3 className="text-lg font-semibold">OSC</h3>

      <div className="mt-2 text-sm md:text-base">
        <p>
          <span className="font-medium">Available Parameters:</span> {availableCount}
        </p>

        <div className="collapse collapse-arrow border border-base-300 bg-base-100 rounded-box mt-1">
          <input type="checkbox" className="peer" />

          <div className="collapse-title font-medium">
            Cached Parameters: {vrcInfo.cached.length}
          </div>

          <div className="collapse-content">
            <ul className="list-disc ml-4 mt-1 space-y-1 text-xs md:text-sm max-h-40 overflow-y-auto pr-2">
              {cached.map(({ path, displayValue, ageMs }) => (
                <li key={path} className="break-words">
                  <span className="font-mono">{path}</span>{" = "}
                  <span className="font-semibold">{displayValue}</span>
                  {" "}
                  <span className="opacity-60">
                    {ageMs !== Number.POSITIVE_INFINITY
                      ? `(${(ageMs / 1000).toFixed(1)}s ago)`
                      : "(no data)"}
                  </span>
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