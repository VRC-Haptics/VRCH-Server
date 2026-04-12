import { createContext, useState, useEffect, useContext, ReactNode } from 'react';
import { commands, VrcInfo } from '../bindings';

interface VrcContextValue {
  vrcInfo: VrcInfo | null;
}

export const VrcContext = createContext<VrcContextValue>({vrcInfo: null});

export const useVrcContext = () => useContext(VrcContext);

export const VrcProvider = ({ children }: { children: ReactNode}) => {
  const [vrcInfo, setVrcInfo] = useState<VrcInfo | null>(null);

  useEffect(() => {
    const fetchVrc = async () => {
      try {
        const newInfo = await commands.getVrcInfo();
        setVrcInfo(newInfo);
      } catch (error) {
        console.error("Fa;iled to fetch devices:", error);
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
