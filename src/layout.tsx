import { Outlet, Link, useLocation } from "react-router-dom";
import { AiOutlineSetting, AiOutlineHome } from "react-icons/ai";
import { IoGameControllerOutline } from "react-icons/io5";
import { LiaAmbulanceSolid } from "react-icons/lia";
import { GrAction } from "react-icons/gr";
import clsx from "clsx";

export default function Layout() {
  const location = useLocation();

  const selectedClass = "text-primary";
  const defaultClass = "w-10 h-7";
  const linkClass =
    "hover:text-primary w-10 h-10 hover:bg-base-300 rounded-md flex items-center justify-center";

  return (
    <div className="absolute inset-0 flex flex-col overflow-hidden">
      <div
        id="settingsBar"
        className="bg-base-200 flex flex-row w-full p-3 gap-3 items-center"
      >
        <Link title="Home" className={linkClass} to="/">
          <AiOutlineHome
            className={clsx(defaultClass, {
              [selectedClass]: location.pathname === "/",
            })}
          />
        </Link>

        <Link title="Games" className={linkClass} to="/game_settings">
          <IoGameControllerOutline
            className={clsx(defaultClass, {
              [selectedClass]: location.pathname === "/game_settings",
            })}
          />
        </Link>

        <div className="flex-grow" />
        <div
          title="Pretty cool project"
          className="text-lg font-bold w-fit text-center"
        >
          VRC Haptics Manager
        </div>
        <div className="flex-grow" />

        <Link
          title="Device Settings"
          className={linkClass}
          to="/device_settings"
        >
          <GrAction
            className={clsx(defaultClass, {
              [selectedClass]: location.pathname === "/device_settings",
            })}
          />
        </Link>

        <Link title="Global Map" className={linkClass} to="/global_map">
          <LiaAmbulanceSolid
            className={clsx(defaultClass, {
              [selectedClass]: location.pathname === "/global_map",
            })}
          />
        </Link>

        <Link title="Settings" className={linkClass} to="/settings">
          <AiOutlineSetting
            className={clsx(defaultClass, {
              //if we are in settings, apply selected class
              [selectedClass]: location.pathname === "/settings",
            })}
          />
        </Link>
      </div>
      <div
        id="windowContainer"
        className="flex flex-col flex-1 overflow-hidden"
      >
        <Outlet />
      </div>
    </div>
  );
}
