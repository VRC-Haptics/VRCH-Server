import React, { useState, useEffect } from "react";
import { Device } from "../../../utils/commonClasses";
import { invoke } from "@tauri-apps/api/core";

interface DeviceOffsetProps {
  selectedDevice: Device;
}

export const DeviceOffset: React.FC<DeviceOffsetProps> = ({ selectedDevice }) => {
  const [multiplier, setMultiplier] = useState<number>(selectedDevice.sens_mult);

  // Update the multiplier if we switch devices
  useEffect(() => {
    setMultiplier(selectedDevice.sens_mult);
  }, [selectedDevice]);

  const handleMultiplierChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = parseFloat(e.target.value);
    setMultiplier(newValue);
    invoke("update_device_multiplier", {mac: selectedDevice.mac, multiplier: multiplier});
  };

  return (
    <div id="SensitivityMultiplier" className="p-2 min-w-full mx-auto">
      <p className="text-md font-bold">Power Limit</p>
      <div className="bg-base-300 rounded-md p-4 flex flex-col gap-4">
        <input 
          type="range" 
          min={0.0} 
          max={1.0} 
          step="0.01"
          value={multiplier} 
          onChange={handleMultiplierChange} 
          className="range range-primary" 
        />
        <p className="text-sm">
          Set your power limit for this device. Lower may be helpful for devices that are battery limited as well as helping with noise.
        </p>
      </div>
    </div>
  );
};

