import { DeviceProvider } from "../context/DevicesContext";
import OtaUpdate from "./device_settings/ota";

export default function DeviceSettings() {
  return (
    <div id="DeviceSettings" className="flex flex-1 p-1 overflow-y-auto">
      <div
        id="FirmwareEditor"
        className="w-full max-h-min rounded-md p-4 border-2 border-base-200 gap-4"
      >
        <p className="text-md font-bold">Update Firmware</p>
        <div id="OtaUpdate" className="w-full max-h-min p-4 gap-4">
          <p className="text-sm font-bold">Over The Air:</p>
          <DeviceProvider>
            <OtaUpdate />
          </DeviceProvider>
        </div>
      </div>
    </div>
  );
}
