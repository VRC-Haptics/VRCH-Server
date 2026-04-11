import { useState } from "react";
import clsx from "clsx";
import { useDeviceContext } from "../../context/DevicesContext";
import { DeviceInfo } from "../../bindings";
import { getDeviceId, getDeviceName } from "../common";

interface ConnectedDevicesProps {
  onSelectDevice: (deviceName: string) => void;
}

export default function ConnectedDevices({
  onSelectDevice,
}: ConnectedDevicesProps) {
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);
  const { devices } = useDeviceContext();

  return (
    <div
      id="ConnectedDevicesContainer"
      className="min-w-fit bg-base-200 rounded-md p-2"
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
      <div className="divider my-0"></div>
      {devices.length === 0 ? (
        <div className="h-max rounded-md px-2 py-1">No Devices Detected</div>
      ) : (
        devices.map((device) => {
          const id = getDeviceId(device);
          const isSelected = selectedDevice === id;
          const deviceClass = clsx(
            "h-max rounded-md px-2 py-1  hover:bg-base-200",
            isSelected
              ? "bg-base-300 border-2 border-dotted border-base-300"
              : "bg-base-100 "
          );

          return (
            <div key={id}>
              <div
                className={deviceClass}
                title={id}
                onClick={() => {
                  onSelectDevice(id);
                  setSelectedDevice(id);
                }}
              >
                {getDeviceName(device)}
              </div>
              <div className="h-0.5" />
            </div>
          );
        })
      )}
    </div>
  );
}