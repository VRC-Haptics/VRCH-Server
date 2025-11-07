import { createContext, useState, useEffect, useContext } from "react";

import { Store } from "@tauri-apps/plugin-store";
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
  theme: localStorage.getItem("theme") || "dark",
  setTheme: () => {},
  wifiDeviceTimeout: parseInt(localStorage.getItem("wifiDeviceTimeout") || "3"),
  setWifiTimeout: () => {},
  repositories: [DEFAULT_REPO],
  updateRepositories: async () => {},
});

export const useSettingsContext = () => useContext(SettingsContext);

export const SettingsProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  // initialise from localStorage (|| fallback)
  const [theme, setTheme] = useState(localStorage.getItem("theme") || "dark");

  const [wifiDeviceTimeout, setWifiTimeout] = useState<number>(3);

  const [repositories, setRepositories] = useState<GitRepo[]>([DEFAULT_REPO]);
  const [store, setStore] = useState<Store | null>(null);

  // Load from store on mount
  useEffect(() => {
    const initStore = async () => {
      const storeInstance = await Store.load("settings.json");
      setStore(storeInstance);

      const saved = await storeInstance.get<GitRepo[]>("repositories");
      if (saved && saved.length > 0) {
        const hasDefault = saved.some(
          (r) => r.owner === DEFAULT_REPO.owner && r.name === DEFAULT_REPO.name
        );
        setRepositories(hasDefault ? saved : [DEFAULT_REPO, ...saved]);
      }
    };
    initStore();
  }, []);

  // side‑effects: keep DOM + storage in sync
  useEffect(() => {
    document.body.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  }, [theme]);

  useEffect(() => {
    localStorage.setItem("wifiDeviceTimeout", wifiDeviceTimeout.toString());
  }, [wifiDeviceTimeout]);

  const updateRepositories = async (repos: GitRepo[]) => {
    if (!store) return;

    await store.set("repositories", repos);
    await store.save();
    setRepositories(repos);
  };

  return (
    <SettingsContext.Provider
      value={{
        theme,
        setTheme,
        wifiDeviceTimeout,
        setWifiTimeout,
        repositories,
        updateRepositories,
      }}
    >
      {children}
    </SettingsContext.Provider>
  );
};
