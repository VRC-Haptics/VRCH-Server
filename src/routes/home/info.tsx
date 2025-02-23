import { useDeviceContext } from "../../context/DevicesContext";
import { AddressGroupsEditor } from "./info/groups";
import RawDeviceInfo from "./info/raw";
import { AddressGroup } from "../../utils/commonClasses"; // Adjust path
import { invoke } from "@tauri-apps/api/core";
import { addressBuilder } from "../../utils/address_builder";
import { TestAddress } from "./info/test_motors";
import { DeviceOffset } from "./info/set_offset";

interface InfoPageProps {
  selectedDevice: string | null;
}

export default function InfoPage({ selectedDevice }: InfoPageProps) {
  // Instead of just `devices`, now we get both
  const { devices } = useDeviceContext();

  function createInfo(mac_address: string) {
    const device = devices.find((d) => d.mac === mac_address);

    if (device != null) {
      // Handler to update the device's AddressGroups in context
      const addGroup = (group: AddressGroup) => {
        device.addr_groups.push(group)
        invoke("update_device_groups", {mac: device.mac, groups: device.addr_groups});
      };

      const rmvGroup = (group: AddressGroup) => {
        const index = device.addr_groups.indexOf(group);
        if (index !== -1) {
          device.addr_groups.splice(index, 1);
        } 
        invoke("update_device_groups", {mac: device.mac, groups: device.addr_groups});
      }

      const fireGroup = (group_name: string, index: number, percentage: number) => {
        const address = addressBuilder(group_name, index);
        invoke("set_address", {address: address, percentage: percentage});
      }

      return (
        <div id="DeviceInfoCard" className="flex flex-col overflow-y-scroll h-full">
          <AddressGroupsEditor 
            addGroup={addGroup}
            rmvGroup={rmvGroup}
            selectedDevice={device}
          />
          <TestAddress fireAddress={fireGroup} selectedDevice={device}></TestAddress>
          <DeviceOffset selectedDevice={device}></DeviceOffset>
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
