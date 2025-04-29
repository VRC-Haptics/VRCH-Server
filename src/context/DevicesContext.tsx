import { createContext, useState, useEffect, useContext, Dispatch, SetStateAction } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { ReactNode } from 'react';
import { Device } from '../utils/commonClasses';

interface DeviceContextValue {
  devices: Device[];
  setDevices: Dispatch<SetStateAction<Device[]>>;
}

export const DeviceContext = createContext<DeviceContextValue>({
  devices: [],
  setDevices: () => {},
});

export const useDeviceContext = () => useContext(DeviceContext);

export const DeviceProvider = ({ children }: { children: ReactNode }) => {
  //real state
  const [internalDevices, setInternalDevices] = useState<Device[]>([]);

  // Wrap the original setter
  const setDevices = async (valueOrUpdater: SetStateAction<Device[]>) => {
    setInternalDevices(valueOrUpdater);
  };

  useEffect(() => {
    let active = true;
    const loop = async () => {
      while (active) {
        try {
          const devs = await invoke<Device[]>("get_device_list");
          setInternalDevices(devs);
        } catch (err) {
          console.error("fetchDevices error:", err);
        }
        // wait 500ms before next iteration
        await new Promise((r) => setTimeout(r, 500));
      }
    };
    loop();
    return () => { active = false; };
  }, []);

  return (
    <DeviceContext.Provider value={{ devices: internalDevices, setDevices }}>
      {children}
    </DeviceContext.Provider>
  );
};
