import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useVrcContext } from "../../context/VrcContext";
import clsx from "clsx";

export default function VrcInfoCard({}) {
  const {vrcInfo} = useVrcContext();

  const [distancePct, setDistancePct] = useState<number>(
    vrcInfo.dist_weight * 100
  );

  useEffect(() => {
    setDistancePct(vrcInfo.dist_weight * 100);
  }, [vrcInfo.dist_weight]);

  const handleDistChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const pct = parseFloat(e.target.value); // 0-100
    setDistancePct(pct);
    invoke("update_vrc_distance_weight", { distanceWeight: pct / 100 });
  };

  const handleVelChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const velPct = parseFloat(e.target.value);   // 0-100
    const distPct = 100 - velPct;               // keep sum = 100
    setDistancePct(distPct);
    invoke("update_vrc_distance_weight", { distanceWeight: distPct / 100 });
  };

  const velocityPct = 100 - distancePct;

  return (
    <div className="flex-shrink-0 flex-col min-w-0 bg-base-200 rounded-md p-2 space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">VRC</h2>
        <div className="w-2"></div>
        <span
          className={clsx(
            "h-3 w-3 rounded-full",
            vrcInfo.vrc_connected ? "bg-emerald-500" : "bg-rose-500"
          )}
          title={vrcInfo.vrc_connected ? "Connected" : "Disconnected"}
        />
      </div>

      <fieldset className="fieldset bg-base-200 border-base-300 rounded-box w-xs border p-4">
        <legend className="fieldset-legend">Feedback Type</legend>

        {/* Distance weight slider */}
        <label className="form-control">
          <span className="label-text mb-1">Distance&nbsp;Weight</span>
          <div className="flex items-center gap-2">
            <input
              type="range"
              min="0"
              max="100"
              step="0.5"
              value={distancePct.toFixed(1)}
              onChange={handleDistChange}
              className="range range-sm flex-1"
            />
            <span className="w-12 text-right tabular-nums">
              {distancePct.toFixed(1)}%
            </span>
          </div>
        </label>

        {/* Velocity weight slider */}
        <label className="form-control mt-4">
          <span className="label-text mb-1">Velocity&nbsp;Weight</span>
          <div className="flex items-center gap-2">
            <input
              type="range"
              min="0"
              max="100"
              step="0.5"
              value={velocityPct.toFixed(1)}
              onChange={handleVelChange}
              className="range range-sm flex-1"
            />
            <span className="w-12 text-right tabular-nums">
              {velocityPct.toFixed(1)}%
            </span>
          </div>
        </label>

        <p className="label-text-alt mt-4">
          What percentage of feedback should be from distance or velocity.
        </p>
      </fieldset>

      {/* Connection details */}
      <div className="mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm md:text-base">
        <span className="font-medium">In&nbsp;Port:</span>
        <span>{vrcInfo.in_port ?? "—"}</span>
        <span className="font-medium">Out&nbsp;Port:</span>
        <span>{vrcInfo.out_port ?? "—"}</span>
        <span className="font-medium">Cache&nbsp;Length:</span>
        <span>{vrcInfo.cache_length}</span>
      </div>

      {/* Avatar section */}
      {vrcInfo.avatar && (
        <div className="mt-6">
          <h3 className="text-lg font-semibold">Avatar</h3>
          <div className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm md:text-base">
            <span className="font-medium">ID:</span>
            <span>{vrcInfo.avatar.id}</span>
            {vrcInfo.avatar.prefab_name && (
              <>
                <span className="font-medium">Prefab:</span>
                <span>{vrcInfo.avatar.prefab_name}</span>
              </>
            )}
            {vrcInfo.avatar.conf && (
              <>
                <span className="font-medium">Map&nbsp;Name:</span>
                <span>{vrcInfo.avatar.conf.meta.map_name}</span>
                <span className="font-medium">Version:</span>
                <span>{vrcInfo.avatar.conf.meta.map_version}</span>
                <span className="font-medium">Author:</span>
                <span>{vrcInfo.avatar.conf.meta.map_author}</span>
                <span className="font-medium">Haptic&nbsp;Nodes:</span>
                <span>{vrcInfo.avatar.conf.nodes.length}</span>
              </>
            )}
          </div>
        </div>
      )}

      {/* OSC summary */}
      <div className="mt-6">
        <h3 className="text-lg font-semibold">OSC</h3>
        <div className="mt-2 text-sm md:text-base">
          <p>
            <span className="font-medium">Available&nbsp;Parameters:</span>{" "}
            {Object.keys(vrcInfo.available_parameters).length}
          </p>
          <p>
            <span className="font-medium">Cached&nbsp;Parameters:</span>{" "}
            {Object.keys(vrcInfo.parameter_cache).length}
          </p>
        </div>
      </div>
    </div>
  );
}