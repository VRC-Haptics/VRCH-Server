import { FC } from "react";
import { ConfNode } from "../../../bindings";

interface HapticNodesSummaryProps {
  nodes: ConfNode[];
}

const shortPath = (p: string) => p.replace(/^\/avatar\/parameters\//, "");

export const HapticNodesSummary: FC<HapticNodesSummaryProps> = ({ nodes }) => {
  const list = nodes ?? [];

  return (
    <div>
      <h3 className="text-lg font-semibold">Haptic Nodes</h3>

      <div className="collapse collapse-arrow border border-base-300 bg-base-100 rounded-box mt-1 text-sm md:text-base">
        <input type="checkbox" className="peer" />

        <div className="collapse-title font-medium">Nodes: {list.length}</div>

        <div className="collapse-content max-h-40 overflow-y-auto">
          <ul className="ml-1 mt-1 space-y-2 text-xs md:text-sm pr-2">
            {list.map((node, idx) => (
              <li key={idx} className="break-words border-b border-base-300/50 pb-2 last:border-0">
                <div className="font-medium">
                  {node.parentBone}{" "}
                  <span className="opacity-60">
                    · {node.interpolationLayer} · r={node.radius.toFixed(3)}
                  </span>
                </div>
                <div className="opacity-60 font-mono text-[0.7rem]">
                  ({node.location.map((n) => n.toFixed(2)).join(", ")})
                </div>

                {node.inputs.length > 0 && (
                  <ul className="list-disc ml-4 mt-1 space-y-0.5">
                    {node.inputs.map((input, i) => (
                      <li key={i} className="break-words">
                        <span className="font-mono">
                          {shortPath(input.vrcPrefix + input.address)}
                        </span>{" "}
                        <span className="opacity-60">
                          ({input.source} · {input.layer})
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
};