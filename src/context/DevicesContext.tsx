import { createContext, useState, useEffect, useContext, Dispatch, SetStateAction, ReactNode } from 'react';
import { commands, DeviceInfo } from '../bindings';

interface DeviceContextValue {
  devices: DeviceInfo[];
  setDevices: Dispatch<SetStateAction<DeviceInfo[]>>;
}

export const DeviceContext = createContext<DeviceContextValue>({
  devices: [],
  setDevices: () => {},
});

export const useDeviceContext = () => useContext(DeviceContext);

export const DeviceProvider = ({ children }: { children: ReactNode }) => {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);

  useEffect(() => {
    let active = true;
    const poll = async () => {
      while (active) {
        try {
          const devs = await commands.getDeviceList();
          const infos = devs
            .map(([_id, info]) => info)
            .filter((info): info is DeviceInfo => info !== null);
          setDevices(infos);
        } catch (err) {
          console.error("fetchDevices error:", err);
        }
        await new Promise((r) => setTimeout(r, 500));
      }
    };
    poll();
    return () => { active = false; };
  }, []);

  return (
    <DeviceContext.Provider value={{ devices, setDevices }}>
      {children}
    </DeviceContext.Provider>
  );
};