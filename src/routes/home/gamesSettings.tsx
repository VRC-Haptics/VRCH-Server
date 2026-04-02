import { useEffect, useState } from "react";
import { useVrcContext } from "../../context/VrcContext";
import { commands } from "../../bindings";
import clsx from "clsx";
import OscSummary from "./gamesSettings/CacheSummary";
import { HapticNodesSummary } from "./gamesSettings/HapticNodes";

// setVrc requires samples & smoothS but VrcInfo doesn't expose them.
// Use sensible defaults — update these if you add those fields to VrcInfo.
const DEFAULT_SAMPLES = 10;
const DEFAULT_SMOOTH: { secs: number; nanos: number } = { secs: 0, nanos: 100_000_000 };

export default function VrcInfoCard() {
  const { vrcInfo } = useVrcContext();

  const [velocityRatio, setVelocityRatio] = useState(0.5);
  const [velocityMult, setVelocityMult] = useState(1.0);

  useEffect(() => {
    if (!vrcInfo) return;
    setVelocityRatio(vrcInfo.velocity_ratio);
    setVelocityMult(vrcInfo.velocity_mult);
  }, [vrcInfo?.velocity_ratio, vrcInfo?.velocity_mult]);

  if (!vrcInfo) return <div className="bg-base-200 rounded-md p-2">Loading VRC info…</div>;

  const distancePct = (1 - velocityRatio) * 100;
  const velocityPct = velocityRatio * 100;

  const pushSettings = (mult: number, ratio: number) => {
    commands.setVrc(mult, ratio, DEFAULT_SAMPLES, DEFAULT_SMOOTH);
  };

  const SNAP_VALUE = 1;
  const SNAP_THRESHOLD = 0.05;

  const handleVelMultChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = parseFloat(e.target.value);
    const snapped = Math.abs(raw - SNAP_VALUE) < SNAP_THRESHOLD ? SNAP_VALUE : raw;
    setVelocityMult(snapped);
    pushSettings(snapped, velocityRatio);
  };

  const handleDistChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const pct = parseFloat(e.target.value);
    const newRatio = 1 - pct / 100;
    setVelocityRatio(newRatio);
    pushSettings(velocityMult, newRatio);
  };

  const handleVelChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newRatio = parseFloat(e.target.value) / 100;
    setVelocityRatio(newRatio);
    pushSettings(velocityMult, newRatio);
  };

  return (
    <div
      id="vrcContainer"
      className="bg-base-200 rounded-md p-2 min-w-0 min-h-0 overflow-hidden h-full flex flex-col"
    >
      <div className="font-bold bg-base-300 rounded-md px-2 py-1 h-min flex items-center">
        <h2>VRC</h2>
        <div className="flex-grow"></div>
        <span
          className={clsx(
            "h-3 w-3 rounded-full",
            vrcInfo.is_connected ? "bg-emerald-500" : "bg-rose-500"
          )}
          title={vrcInfo.is_connected ? "Connected" : "Disconnected"}
        />
      </div>

      <div className="divider my-0" />

      <div id="vrcInfo" className="max-w-full max-h-full overflow-auto">
        <fieldset className="fieldset bg-base-200 border-base-300 rounded-box w-xs border p-4">
          <legend className="fieldset-legend">Feedback Type</legend>

          <label className="form-control">
            <span className="label-text mb-1">Distance&nbsp;Weight</span>
            <div className="flex items-center gap-2">
              <input type="range" min="0" max="100" step="0.5"
                value={distancePct.toFixed(1)} onChange={handleDistChange}
                className="range range-sm flex-1" />
              <span className="w-12 text-right tabular-nums">{distancePct.toFixed(1)}%</span>
            </div>
          </label>

          <label className="form-control mt-4">
            <span className="label-text mb-1">Velocity&nbsp;Weight</span>
            <div className="flex items-center gap-2">
              <input type="range" min="0" max="100" step="0.5"
                value={velocityPct.toFixed(1)} onChange={handleVelChange}
                className="range range-sm flex-1" />
              <span className="w-12 text-right tabular-nums">{velocityPct.toFixed(1)}%</span>
            </div>
          </label>

          <p className="label-text-alt mt-4">
            What percentage of feedback should be from distance or velocity.
          </p>
          <div className="h-3"></div>

          <label className="form-control">
            <span>Velocity scaling</span>
            <div className="flex items-center gap-2">
              <input type="range" min="0" max="2" step="0.01"
                value={velocityMult} onChange={handleVelMultChange}
                className="range range-sm flex-1" />
              <span className="w-12 text-right tabular-nums">{velocityMult.toFixed(2)}x</span>
            </div>
          </label>

          <p className="label-text-alt mt-4">
            How much feedback is given for an in-game speed. Higher values reach max feedback at lower speeds.
          </p>
        </fieldset>

        <div className="mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm md:text-base">
          <span className="font-medium">In&nbsp;Port:</span>
          <span>{vrcInfo.in_port ?? "—"}</span>
          <span className="font-medium">Out&nbsp;Port:</span>
          <span>{vrcInfo.out_port ?? "—"}</span>
        </div>

        {vrcInfo.avatar && (
          <div className="mt-6">
            <h3 className="text-lg font-semibold">Avatar</h3>
            <div className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm md:text-base">
              <span className="font-medium">ID:</span>
              <span>{vrcInfo.avatar.id ?? "None"}</span>
              {vrcInfo.avatar.prefab_names.length > 0 && (
                <>
                  <span className="font-medium">Prefabs:</span>
                  <span>{vrcInfo.avatar.prefab_names.join(", ")}</span>
                </>
              )}
              {vrcInfo.avatar.configs.length > 0 && (() => {
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