import { createContext, useState, useEffect, useContext } from "react";
import { commands } from "../bindings";
import { GitRepo } from "../utils/commonClasses";

export const DEFAULT_REPO = { owner: "VRC-Haptics", name: "VRCH-Firmware" };

interface SettingsContextInterface {
  theme: string;
  setTheme: (theme: string) => void;
  wifiDeviceTimeout: number;
  setWifiTimeout: (timeout: number) => void;
  repositories: GitRepo[];
  updateRepositories: (repos: GitRepo[]) => Promise<void>;
}

export const SettingsContext = createContext<SettingsContextInterface>({
  theme: "dark",
  setTheme: () => {},
  wifiDeviceTimeout: 3,
  setWifiTimeout: () => {},
  repositories: [DEFAULT_REPO],
  updateRepositories: async () => {},
});

export const useSettingsContext = () => useContext(SettingsContext);

export const SettingsProvider = ({ children }: { children: React.ReactNode }) => {
  const [theme, setThemeState] = useState(localStorage.getItem("theme") || "dark");
  const [wifiDeviceTimeout, setWifiTimeoutState] = useState<number>(3);
  const [repositories, setRepositories] = useState<GitRepo[]>([DEFAULT_REPO]);

  useEffect(() => {
    commands.getRepositories().then((repos) => {
      if (repos.length > 0) setRepositories(repos);
    });
    commands.getWifiTimeout().then(setWifiTimeoutState);
  }, []);

  useEffect(() => {
    document.body.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  }, [theme]);

  const setTheme = (t: string) => setThemeState(t);

  const setWifiTimeout = (timeout: number) => {
    setWifiTimeoutState(timeout);
    commands.setWifiTimeout(timeout);
  };

  const updateRepositories = async (repos: GitRepo[]) => {
    await commands.setRepositories(repos);
    setRepositories(repos);
  };

  return (
    <SettingsContext.Provider
      value={{ theme, setTheme, wifiDeviceTimeout, setWifiTimeout, repositories, updateRepositories }}
    >
      {children}
    </SettingsContext.Provider>
  );
};