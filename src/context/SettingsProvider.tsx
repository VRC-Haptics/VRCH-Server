import {
  createContext,
  useState,
  useEffect,
  useContext,
} from "react";

interface SettingsContextInterface {
  theme: string;
  setTheme: (theme: string) => void;
  wifiDeviceTimeout: number;
  setWifiTimeout: (timeout: number) => void;
}

export const SettingsContext = createContext<SettingsContextInterface>({
  theme: localStorage.getItem("theme") || "dark",
  setTheme: () => {},
  wifiDeviceTimeout: parseInt(localStorage.getItem("wifiDeviceTimeout") || "3"),
  setWifiTimeout: () => {},
});

export const useSettingsContext = () => useContext(SettingsContext);

export const SettingsProvider = ({ children }: { children: React.ReactNode }) => {
  // initialise from localStorage (|| fallback)
  const [theme, setTheme] = useState(
    localStorage.getItem("theme") || "dark"
  );

  const [wifiDeviceTimeout, setWifiTimeout] = useState<number>(3);

  // side‑effects: keep DOM + storage in sync
  useEffect(() => {
    document.body.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  }, [theme]);

  useEffect(() => {
    localStorage.setItem("wifiDeviceTimeout", wifiDeviceTimeout.toString());
  }, [wifiDeviceTimeout]);

  return (
    <SettingsContext.Provider
      value={{
        theme,
        setTheme,
        wifiDeviceTimeout,
        setWifiTimeout,
      }}
    >
      {children}
    </SettingsContext.Provider>
  );
};

