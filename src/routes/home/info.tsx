import { useDeviceContext } from "../../context/DevicesContext";
import RawDeviceInfo from "./info/raw";
import DeviceJsonUpload from "./info/upload_map";
import { DeviceOffset } from "./info/set_offset";
import { DisplayHapticNodes } from "./info/display_haptic_nodes";

interface InfoPageProps {
  selectedDevice: string | null;
}

export default function InfoPage({ selectedDevice }: InfoPageProps) {
  const { devices } = useDeviceContext();

  function createInfo(device_id: string) {
    const device = devices.find((d) => d.id === device_id);

    if (device != null) {
      return (
        <div
          id="DeviceInfoCard"
          className="flex flex-col min-w-0 max-w-full overflow-y-scroll"
        >
          <DeviceOffset selectedDevice={device}></DeviceOffset>
          <div className="h-2"></div>
          <DisplayHapticNodes selectedDevice={device}></DisplayHapticNodes>
          <div className="h-2"></div>
          <DeviceJsonUpload device={device}></DeviceJsonUpload>
          <div className="h-2"></div>
          <RawDeviceInfo device={device} />
        </div>
      );
    }
  }

  return (
    <div
      id="infoPageContainer"
      className="bg-base-200 rounded-md p-2 min-w-0 min-h-0"
    >
      <div className="font-bold bg-base-300 rounded-md px-2 py-1 h-min">
        <h1>Device Info</h1>
      </div>
      <div className="divider my-0"></div>
      {selectedDevice ? (
        createInfo(selectedDevice)
      ) : (
        <div id="defaultInfoCard" className="text-center">
          <h1 className="text-lg">Welcome To VRC Haptics!</h1>
          <p>
            Make sure you device is connected to the same wifi network and then
            select it from the connected devices tab. Your device info will pop
            up here.
          </p>
        </div>
      )}
    </div>
  );
}
