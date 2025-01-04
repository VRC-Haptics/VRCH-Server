import { Outlet } from "react-router-dom";
import themes from "../utils/themes";
import { useSettingsContext } from "../context/SettingsProvider";

// Define the types for the settings
type SettingType = 'toggle' | 'dropdown';

interface Setting {
  title: string;
  help: string;
  type: SettingType;
  getFunction:  string | number | readonly string[] | undefined;
  setFunction: (value: string) => void;
  options?: string[]; // Only for dropdown type
}

interface Group {
  title: string;
  help: string;
}

// Component for individual settings item
const SettingsItem: React.FC<Setting> = ({ title, help, type, getFunction, setFunction, options }) => {
  return (
    <div title= {help} className="flex flex-col p-2">
      <h3 className="font-semibold text-lg">{title}</h3>
      <h6 className="text-info text-sm p-1">{help}</h6>

      <div className="max-h-min rounded-md bg-base-200 p-1">
        {type === 'toggle' ? (
          <div className="form-control left">
            <label className="label cursor-pointer">
              <input type="checkbox" className="toggle-primary" value={getFunction} onChange={(e)=> setFunction(e.target.value)}/>
            </label>
          </div>
        ) : (
          <select className="select-primary rounded-md select-bordered" value={getFunction} onChange={(e) => setFunction(e.target.value)}>
            {options?.map((option, index) => (
              <option key={index} value={option}>
                {option}
              </option>
            ))}
          </select>
        )}
      </div>
    </div>
  );
};

// Component for a group of settings
const SettingsGroup: React.FC<{ group: Group, settings: Setting[] }> = ({ group, settings }) => {
  return (
    <div id= {group.title} className="h-fit w-full px-4">
      <div className="text-md font-bold w-fit text-center text-lg">
        <h2 title={group.help}>{group.title}</h2>
      </div>
      <div className="flex flex-col h-fit justify-center p-1 w-full">  
        {settings.map((setting, index) => (
          <SettingsItem key={index} {...setting} />
        ))}
      </div>
    </div>
  );
};



export default function Settings() {
  const { setTheme, theme: currentTheme } = useSettingsContext();

  // app settings group
  const appGroup: Group = { title: 'App Settings', help: 'Settings for the app' };
  const appData: Setting[] = [
    { title: 'Theme', help: 'Select your preferred theme', type: 'dropdown', options: themes, getFunction: currentTheme, setFunction: setTheme },
  ];

  return (
    <div className="flex flex-col h-full w-full">
      <h1 className="text-2xl font-bold padding-5 text-center">
        Settings
      </h1>

      <SettingsGroup group={appGroup} settings={appData} />
    </div>
  );
}
