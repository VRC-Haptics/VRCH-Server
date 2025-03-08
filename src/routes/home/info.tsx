import { useDeviceContext } from "../../context/DevicesContext";
import RawDeviceInfo from "./info/raw";
import DeviceJsonUpload from "./info/upload_map";
import { DeviceOffset } from "./info/set_offset";

interface InfoPageProps {
  selectedDevice: string | null;
}

export default function InfoPage({ selectedDevice }: InfoPageProps) {
  // Instead of just `devices`, now we get both
  const { devices } = useDeviceContext();

  function createInfo(device_id: string) {
    const device = devices.find((d) => d.id === device_id);

    if (device != null) {
      return (
        <div id="DeviceInfoCard" className="flex flex-col overflow-y-scroll h-full">
          {//<TestAddress fireAddress={fireGroup} selectedDevice={device}></TestAddress>
          }
          <DeviceOffset selectedDevice={device}></DeviceOffset>
          <DeviceJsonUpload device={device}></DeviceJsonUpload>
          <div className="flex-grow"></div>
          <RawDeviceInfo device={device} />
        </div>
      );
    }
  }

  return (
    <div
      id="infoPageContainer"
      className="flex flex-col h-full w-full bg-base-200 rounded-md p-2 space-y-2"
    >
      <div className="flex font-bold bg-base-300 rounded-md px-2 py-1 w-full h-min">
        <h1>Device Info</h1>
      </div>
      <div
        id="infoElements"
        className="w-full h-full border-4 border-dotted rounded-md border-base-300"
      >
        {selectedDevice ? (
          createInfo(selectedDevice)
        ) : (
          <div id="defaultInfoCard" className="text-center">
            <h1 className="text-lg">Welcome To VRC Haptics!</h1>
            <p>
              Make sure you device is connected to the same wifi network and
              then select it from the connected devices tab. Your device info
              will pop up here.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
