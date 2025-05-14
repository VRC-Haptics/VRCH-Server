import { createContext, useState, useEffect, useContext, ReactNode } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { defaultVrcInfo, VrcInfo } from '../utils/vrc_info_classes';

interface VrcContextValue {
  vrcInfo: VrcInfo;
}

export const VrcContext = createContext<VrcContextValue>({vrcInfo: defaultVrcInfo});

export const useVrcContext = () => useContext(VrcContext);

export const VrcProvider = ({ children }: { children: ReactNode}) => {
  const [vrcInfo, setVrcInfo] = useState<VrcInfo>(defaultVrcInfo);

  useEffect(() => {
    const fetchVrc = async () => {
      try {
        const newInfo = await invoke<VrcInfo>('get_vrc_info');
        setVrcInfo(newInfo);
      } catch (error) {
        console.error("Failed to fetch devices:", error);
      }
    };

    // Initial fetch
    fetchVrc();

    // Polling interval
    const intervalId = setInterval(fetchVrc, 1000);
    return () => clearInterval(intervalId);
  }, []);

  return (
    <VrcContext.Provider value={{vrcInfo: vrcInfo}}>
      {children}
    </VrcContext.Provider>
  );
};
