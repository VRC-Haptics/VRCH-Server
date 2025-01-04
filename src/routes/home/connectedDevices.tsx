import { useState } from "react";
import clsx from "clsx";
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { Device } from "../../utils/commonClasses"

interface ConnectedDevicesProps {
  onSelectDevice: (deviceName: string) => void;
}

export default function ConnectedDevices({
  onSelectDevice,
}: ConnectedDevicesProps) {
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);

  useEffect(() => {
    // Fetch the device list from Rust
    invoke<Device[]>('get_device_list')
      .then((deviceList) => {
        setDevices(deviceList);
      })
      .catch((error) => {
        console.error("Failed to fetch devices:", error);
      });
  });

  return (
    <div
      id="ConnectedDevicesContainer"
      className="flex flex-col min-w-fit bg-base-200 rounded-md p-2 space-y-2"
    >
      <div
        className="font-bold bg-base-300 rounded-md px-2 py-1 h-min"
        onClick={() => {
          onSelectDevice("");
          setSelectedDevice("");
        }}
      >
        <h1>Connected Devices</h1>
      </div>
      {devices.length === 0 ? (
        <div className="h-max rounded-md px-2 py-1">
          No Devices Detected
        </div>
      ) : (
        devices.map((device) => {
          const isSelected = selectedDevice === device.MAC;
          const deviceClass = clsx(
            "h-max rounded-md px-2 py-1",
            isSelected
              ? "bg-base-300 border-2 border-dotted border-base-300"
              : "bg-base-100"
          );

          return (
            <div
              key={device.MAC}
              className={deviceClass}
              onClick={() => {
                onSelectDevice(device.MAC);
                setSelectedDevice(device.MAC);
              }}
            >
              {device.DisplayName}
            </div>
          );
        })
      )}
    </div>
  );
}
