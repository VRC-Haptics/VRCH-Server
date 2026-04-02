import React, { useState, useEffect, useMemo } from "react";
import { commands, DeviceId, DeviceInfo } from "../../../bindings";

interface DeviceOffsetProps {
  deviceId: DeviceId;
  selectedDevice: DeviceInfo;
}

export const DeviceOffset: React.FC<DeviceOffsetProps> = ({ deviceId, selectedDevice }) => {
  const info = selectedDevice.value;
  const [multiplier, setMultiplier] = useState<number>(info.intensity ?? 1.0);
  const [offset, setOffset] = useState<number>(info.offset ?? 0.0);

   useEffect(() => {
    setMultiplier(info.intensity ?? 1.0);
    setOffset(info.offset ?? 0.0);
  }, [info]);


  const multiplierPct = useMemo(() => Math.round(multiplier * 100), [multiplier]);
  const offsetPct = useMemo(() => Math.round(offset * 100), [offset]);

  const handleMultiplierChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = parseFloat(e.target.value);
    setMultiplier(newValue);
    commands.updateDeviceMultiplier(deviceId, newValue);
  };

  const handleOffsetChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = parseFloat(e.target.value);
    setOffset(newValue);
    commands.updateDeviceOffset(deviceId, newValue);
  };

  return (
    <div id="DeviceSettings" className="min-w-full">
      <p className="text-md font-bold">Power Limit</p>
      <div className="bg-base-300 rounded-md p-4 flex flex-col gap-4">
        <div className="flex items-center gap-4">
          <input type="range" min={0} max={1} step="0.001"
            value={multiplier} onChange={handleMultiplierChange}
            className="grow range range-primary" />
          <span className="w-12 text-right tabular-nums">{multiplierPct}%</span>
        </div>
        <p className="text-sm">
          Set your power limit for this device. Game signals are still scaled correctly, they just target this value instead of 100%.
        </p>
      </div>

      <p className="text-md font-bold">Starting Offset</p>
      <div className="bg-base-300 rounded-md p-4 flex flex-col gap-4">
        <div className="flex items-center gap-4">
          <input type="range" min={0} max={1} step="0.001"
            value={offset} onChange={handleOffsetChange}
            className="grow range range-secondary" />
          <span className="w-12 text-right tabular-nums">{offsetPct}%</span>
        </div>
        <p className="text-sm">
          Sets the deadzone that occurs before haptic motors actually start spinning.
        </p>
      </div>
    </div>
  );
};
