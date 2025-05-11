// import { Outlet } from "react-router-dom";
import themes from "../utils/themes";
import { useSettingsContext } from "../context/SettingsProvider";

type SettingType = "toggle" | "dropdown" | "int" | "float";

export interface Setting<T = string | number | boolean> {
  title: string;
  help:  string;
  type:  SettingType;
  defaultValue: T;
  getFunction: T;                  // current value
  setFunction: (value: T) => void; // updater
  options?: string[];              // <‑‑ dropdown‑only
}

interface Group {
  title: string;
  help:  string;
}

/// A single Settings item. 
function SettingsItem<T extends string | number | boolean>({
  title,
  help,
  type,
  defaultValue,
  getFunction,
  setFunction,
  options,
}: Setting<T>) {
  const numberHandler = (raw: string) => {
    const parsed =
      type === "int" ? parseInt(raw, 10) :
      type === "float" ? parseFloat(raw) :
      (raw as unknown as T);

    if (typeof parsed === "number" ? !Number.isNaN(parsed) : true) {
      setFunction(parsed as T);
    }
  };

  return (
    <div title={help} className="flex flex-col p-2 bg-base-200 rounded-md">
      <h3 className="font-semibold text-lg">{title}</h3>
      <h6 className="text-info text-sm p-1">{help}</h6>

      <div className="max-h-min rounded-md p-1">
        {type === "toggle" && (
            <label className="label cursor-pointer">
            <input
              type="checkbox"
              className="toggle toggle-primary"
              checked={Boolean(getFunction)}
              onChange={e => setFunction(e.target.checked as T)}
            />
            <span className="text-xs opacity-70">(Default {String(defaultValue)})</span>
          </label>
          
        )}

        {type === "dropdown" && (
          <select
            className="select-s select-primary select-bordered rounded"
            value={String(getFunction)}
            onChange={e => setFunction(e.target.value as T)}
          >
            {options?.map(o => (
              <option key={o} value={o}>{o}</option>
            ))}
          </select>
        )}

        {(type === "int" || type === "float") && (
          <>
          <input
            type="number"
            className="input input-primary input-sm w-20 text-right"
            value={String(getFunction)}
            step={type === "int" ? 1 : "any"}
            inputMode={type === "int" ? "numeric" : "decimal"}
            pattern={type === "int" ? "\\d+" : undefined}
            onChange={e => numberHandler(e.target.value)}
          />
          <p className="text-xs opacity-70 mt-1">Default: {String(defaultValue)}</p>
        </>
        )}
      </div>
    </div>
  );
}

function SettingsGroup<T extends string | number | boolean>({
  group,
  settings,
}: {
  group: Group;
  settings: Setting<T>[];
}) {
  return (
    <div id={group.title} className="h-fit w-full px-4">
      <div className="text-md font-bold w-fit text-center text-lg">
        <h2 title={group.help}>{group.title}</h2>
      </div>
      <div className="flex flex-col h-fit justify-center p-1 w-full">
        {settings.map((s, i) => (
          <SettingsItem key={i} {...s} />
        ))}
      </div>
    </div>
  );
}

export default function Settings() {
  const {
    theme: currentTheme,
    setTheme,
    wifiDeviceTimeout,
    setWifiTimeout,
  } = useSettingsContext();

  /* App‑level settings */
  const appGroup: Group = { title: "App Settings", help: "Settings for the app" };
  const appData: Setting<string>[] = [
    {
      title: "Theme",
      help: "Select your preferred theme",
      type: "dropdown",
      defaultValue: "dark",
      options: themes,
      getFunction: currentTheme,
      setFunction: setTheme,
    },
  ];

  /* Wi‑Fi device settings */
  const wifiGroup: Group = {
    title: "Wifi Device Settings",
    help: "Settings for devices connected via Wifi.",
  };
  const wifiData: Setting<number>[] = [
    {
      title: "Timeout (s)",
      help:
        "Seconds until a device is considered disconnected. Raise this if devices keep dropping out.",
      type: "int",
      defaultValue: 3,
      getFunction: wifiDeviceTimeout,
      setFunction: setWifiTimeout,
    },
  ];

  return (
    <div className="flex flex-col h-full w-full">
      <h1 className="text-2xl font-bold padding-5 text-center">Settings</h1>

      {/* generic inference keeps each group type‑safe */}
      <SettingsGroup group={appGroup}  settings={appData} />
      <SettingsGroup group={wifiGroup} settings={wifiData} />
    </div>
  );
}