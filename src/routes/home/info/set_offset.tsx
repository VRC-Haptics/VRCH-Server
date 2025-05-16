import React, { useState, useEffect, useMemo } from "react";
import { Device } from "../../../utils/commonClasses";
import { invoke } from "@tauri-apps/api/core";

interface DeviceOffsetProps {
  selectedDevice: Device;
}

export const DeviceOffset: React.FC<DeviceOffsetProps> = ({ selectedDevice }) => {
  const [multiplier, setMultiplier] = useState<number>(
    selectedDevice.factors.sens_mult,
  );

  const [offset, setOffset] = useState<number>(
    selectedDevice.factors.start_offset,
  );

  useEffect(() => {
    setMultiplier(selectedDevice.factors.sens_mult);
    setOffset(selectedDevice.factors.start_offset);
  }, [selectedDevice]);

  const multiplierPct = useMemo(() => Math.round(multiplier * 100), [multiplier]);
  const offsetPct = useMemo(() => Math.round(offset * 100), [offset]);

  const handleMultiplierChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = parseFloat(e.target.value);
    setMultiplier(newValue);
    invoke("update_device_multiplier", {
      deviceId: selectedDevice.id,
      multiplier: newValue,
    });
  };

  const handleOffsetChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = parseFloat(e.target.value);
    setOffset(newValue);
    invoke("update_device_offset", {
      deviceId: selectedDevice.id,
      offset: newValue,
    });
  };

  return (
    <div id="DeviceSettings" className="p-2 min-w-full mx-auto">

      <p className="text-md font-bold">Power Limit</p>
      <div className="bg-base-300 rounded-md p-4 flex flex-col gap-4">
        <div className="flex items-center gap-4">
          <input
            type="range"
            min={0}
            max={1}
            step="0.001"
            value={multiplier}
            onChange={handleMultiplierChange}
            className="grow range range-primary"
          />
          <span className="w-12 text-right tabular-nums">{multiplierPct}%</span>
        </div>
        <p className="text-sm">
          Set your power limit for this device. Game signals are still scaled correctly, they just target this value instead of 100%. Lower may be helpful for devices
          that are battery-limited as well as helping with noise and discomfort.
        </p>
      </div>

      <p className="text-md font-bold">Starting Offset</p>
      <div className="bg-base-300 rounded-md p-4 flex flex-col gap-4">
        <div className="flex items-center gap-4">
          <input
            type="range"
            min={0.0}
            max={1.0}
            step="0.001"
            value={offset}
            onChange={handleOffsetChange}
            className="grow range range-secondary"
          />
          <span className="w-12 text-right tabular-nums">
            {offsetPct}%
          </span>
        </div>
        <p className="text-sm">
          Sets the deadzone that occurs before haptic motors actually start spinning. Essentially the lowest haptic motors will go.
        </p>
      </div>
    </div>
  );
};
