import { FC } from "react";
import { VrcInfo } from "../../../utils/vrc_info_classes";

interface OscSummaryProps {
  vrcInfo: VrcInfo
}

type TimestampPair = { secs_since_epoch: number; nanos_since_epoch: number };

const OscSummary: FC<OscSummaryProps> = ({ vrcInfo: vrcInfo }) => {
  const availableCount = Object.keys(vrcInfo.available_parameters).length;
  const cachedEntries = Object.entries(vrcInfo.parameter_cache);

  const tsPairToMs = (t: TimestampPair) => t.secs_since_epoch * 1000 + t.nanos_since_epoch / 1e6;

  /** Build array with age (ms) already calculated, then sort newest-first */
  const cached = Object.entries(vrcInfo.parameter_cache).map(([path, node]) => {
    // Each ring-buffer entry is [ oscValue, protobufTimestamp ]
    const entry = node.values[0];                       // may be undefined
    let ageMs = Number.POSITIVE_INFINITY;

    if (Array.isArray(entry) && entry.length === 2) {
      const ts = entry[1] as TimestampPair;             // index 1 holds the timestamp
      ageMs = Date.now() - tsPairToMs(ts);
    }

    return { path, ageMs };
  });

  cached.sort((a, b) => a.ageMs - b.ageMs);  

  return (
    <div>
      <h3 className="text-lg font-semibold">OSC</h3>

      <div className="mt-2 text-sm md:text-base">
        {/* Available parameters (simple count) */}
        <p>
          <span className="font-medium">Available Parameters:</span> {availableCount}
        </p>

        {/* Cached parameters (count + DaisyUI collapse list) */}
        <div className="collapse collapse-arrow border border-base-300 bg-base-100 rounded-box mt-1">
          {/* The checkbox is required by DaisyUI to manage the collapsed state */}
          <input type="checkbox" className="peer" />

          <div className="collapse-title font-medium">
            Cached Parameters: {cachedEntries.length}
          </div>

          <div className="collapse-content">
            <ul className="list-disc ml-4 mt-1 space-y-1 text-xs md:text-sm max-h-40 overflow-y-auto pr-2">
              {cached.map(({ path, ageMs }) => (
                <li key={path} className="break-words">
                  {path}:{" "}
                  {ageMs !== Number.POSITIVE_INFINITY
                    ? (ageMs / 1000).toFixed(2) + "s"
                    : "- -"}
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
