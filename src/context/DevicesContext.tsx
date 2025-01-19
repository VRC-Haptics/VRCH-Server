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

    // invoke rust
    try {
      await invoke('invalidate_cache');
    } catch (err) {
      console.error("Failed to invalidate address cache:", err);
    }
  };

  useEffect(() => {
    const fetchDevices = async () => {
      try {
        const deviceList = await invoke<Device[]>('get_device_list');
        setInternalDevices(deviceList);
      } catch (error) {
        console.error("Failed to fetch devices:", error);
      }
    };

    // Initial fetch
    fetchDevices();

    // Polling interval
    const intervalId = setInterval(fetchDevices, 500);
    return () => clearInterval(intervalId);
  }, []);

  return (
    <DeviceContext.Provider value={{ devices: internalDevices, setDevices }}>
      {children}
    </DeviceContext.Provider>
  );
};
