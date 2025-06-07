import { FC } from "react";
import { ConfNode } from "../../../utils/vrc_info_classes";

interface HapticNodesSummaryProps {
  nodes: ConfNode[];
}

export const HapticNodesSummary: FC<HapticNodesSummaryProps> = ({ nodes }) => {
  const nodeCount = nodes.length;

  return (
    <div>
      <h3 className="text-lg font-semibold">Haptic Nodes</h3>

      {/* Collapsible, scrollable node list */}
      <div className="collapse collapse-arrow border border-base-300 bg-base-100 rounded-box mt-1 max-h-60 text-sm md:text-base">
        <input type="checkbox" className="peer" />

        <div className="collapse-title font-medium">Nodes: {nodeCount}</div>

        <div className="collapse-content max-h-full">
          <ul className="list-disc ml-4 mt-1 space-y-1 text-xs md:text-sm max-h-full overflow-y-auto pr-2">
            {nodes.map((node, idx) => (
              <li key={idx} className="break-words">
                <span className="font-medium">{node.address ?? `Node ${idx + 1}`}</span>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
};