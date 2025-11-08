import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useVrcContext } from "../../context/VrcContext";
import clsx from "clsx";
import OscSummary from "./gamesSettings/CacheSummary";
import { HapticNodesSummary } from "./gamesSettings/HapticNodes";

export default function VrcInfoCard({}) {
  const { vrcInfo } = useVrcContext();

  const [distancePct, setDistancePct] = useState<number>(
    vrcInfo.dist_weight * 100
  );

  const [velocityMult, setVelocityMult] = useState<number>(
    vrcInfo.vel_multiplier
  );

  useEffect(() => {
    setDistancePct(vrcInfo.dist_weight * 100);
  }, [vrcInfo.dist_weight]);

  useEffect(() => {
    setVelocityMult(vrcInfo.vel_multiplier);
  }, [vrcInfo.vel_multiplier]);

  const SNAP_VALUE = 1;
  const SNAP_THRESHOLD = 0.05;

  const handleVelMultChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = parseFloat(e.target.value);
    const snapped =
      Math.abs(raw - SNAP_VALUE) < SNAP_THRESHOLD ? SNAP_VALUE : raw;
    setVelocityMult(snapped);
    invoke("update_vrc_velocity_multiplier", { velMultiplier: snapped });
  };

  const handleDistChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const pct = parseFloat(e.target.value); // 0-100
    setDistancePct(pct);
    invoke("update_vrc_distance_weight", { distanceWeight: pct / 100 });
  };

  const handleVelChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const velPct = parseFloat(e.target.value); // 0-100
    const distPct = 100 - velPct; // keep sum = 100
    setDistancePct(distPct);
    invoke("update_vrc_distance_weight", { distanceWeight: distPct / 100 });
  };

  const velocityPct = 100 - distancePct;

  return (
    <div
      id="vrcContainer"
      className="bg-base-200 rounded-md p-2 min-w-0 min-h-0 overflow-hidden h-full flex flex-col"
    >
      {/* Header */}
      <div className="font-bold bg-base-300 rounded-md px-2 py-1 h-min">
        <h2>VRC</h2>
        <div className="w-2"></div>
        <span
          className={clsx(
            "h-3 w-3 rounded-full",
            vrcInfo.vrc_connected ? "bg-emerald-500" : "bg-rose-500"
          )}
          title={vrcInfo.vrc_connected ? "Connected" : "Disconnected"}
        />
      </div>

      <div className="divider my-0" />

      <div id="vrcInfo" className="max-w-full max-h-full overflow-auto">
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
          <div className="h-3"></div>

          <label className="form-control">
            <span>Velocity scaling</span>
            <div className="flex items-center gap-2">
              <input
                type="range"
                min="0"
                max="2"
                step="0.01"
                value={velocityMult}
                onChange={handleVelMultChange}
                className="range range-sm flex-1"
              />
              <span className="w-12 text-right tabular-nums">
                {velocityMult.toFixed(1)}%
              </span>
            </div>
          </label>

          <p className="label-text-alt mt-4">
            How much feedback is given for an in-game speed. Higher values reach
            max feeback at lower speeds.
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
              <span>{vrcInfo.avatar.id ?? "None"}</span>
              {vrcInfo.avatar.prefab_names &&
                vrcInfo.avatar.prefab_names.length > 0 && (
                  <>
                    <span className="font-medium">Prefabs:</span>
                    <span>{vrcInfo.avatar.prefab_names.join(", ")}</span>
                  </>
                )}
              {vrcInfo.avatar.configs &&
                vrcInfo.avatar.configs.length > 0 &&
                (() => {
                  const conf = vrcInfo.avatar!.configs[0];
                  return (
                    <>
                      <span className="font-medium">Map Name:</span>
                      <span>{conf.meta.map_name}</span>
                      <span className="font-medium">Version:</span>
                      <span>{conf.meta.map_version}</span>
                      <span className="font-medium">Author:</span>
                      <span>{conf.meta.map_author}</span>
                      <div className="col-span-2 text-sm md:text-base mt-2">
                        <OscSummary vrcInfo={vrcInfo} />
                      </div>
                      <div className="col-span-2 mt-2">
                        <HapticNodesSummary nodes={conf.nodes} />
                      </div>
                    </>
                  );
                })()}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
