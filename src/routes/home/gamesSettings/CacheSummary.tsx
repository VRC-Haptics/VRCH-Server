import { FC } from "react";
import { VrcInfo } from "../../../utils/vrc_info_classes";

interface OscSummaryProps {
  vrcInfo: VrcInfo
}

// This version leverages DaisyUI's built‑in `collapse` component instead of manual state
// management + icon toggling. DaisyUI automatically adds a rotating arrow for the
// expanded/collapsed state and handles the open/close animation.

const OscSummary: FC<OscSummaryProps> = ({ vrcInfo: vrcInfo }) => {
  const availableCount = Object.keys(vrcInfo.available_parameters).length;
  const cachedCount = Object.keys(vrcInfo.parameter_cache).length;

  return (
    <div className="mt-6">
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
            Cached Parameters: {cachedCount}
          </div>

          <div className="collapse-content">
            <ul className="list-disc ml-4 mt-1 space-y-1 text-xs md:text-sm max-h-40 overflow-y-auto pr-2">
              {Object.entries(vrcInfo.parameter_cache).map(([key]) => (
                <li key={key} className="break-words">
                  <span className="font-medium">{key}</span>
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
