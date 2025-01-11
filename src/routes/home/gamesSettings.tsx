import { useContext } from "react";
import { VrcContext } from "../../context/VrcContext";

export default function GameSettings() {
  const vrcInfo = useContext(VrcContext);

  return (
    <div
      id="GameSettingsContainer"
      className="flex flex-col min-w-fit h-full bg-base-200 rounded-md p-2"
    >
      <div className="font-bold bg-base-300 rounded-md px-2 py-1 h-min">
        <h1>Game Settings</h1>
      </div>
      <div className="rounded-md px-2 w-fit py-1 h-full overflow-scroll">
        {Object.keys(vrcInfo.raw_parameters).length === 0 ? (
          <div> Game Not connected</div>
        ) : (
          <div id="vrcAvatarSettings">
            <p>Menu Parameters</p>
            {vrcInfo.avatar?.menu_parameters?.map((parameter) => (
              <div key={parameter.address}>
                <span>{parameter.address}:{parameter.value}</span>
              </div>
            ))}
            <p>Haptics Parameters</p>
            {vrcInfo.avatar?.haptic_parameters?.map((parameter) => (
              <div key={parameter.address}>
                <span>{parameter.address}:{parameter.value}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

