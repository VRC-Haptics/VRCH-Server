import { Outlet, Link, useLocation } from "react-router-dom";
import { AiOutlineSetting, AiOutlineHome, } from "react-icons/ai";
import { IoGameControllerOutline } from "react-icons/io5";
import clsx from "clsx";

export default function Layout() {
  const location = useLocation();

  const selectedClass = "text-primary";
  const defaultClass = "w-10 h-7";
  const linkClass = "hover:text-primary w-10 h-10 hover:bg-base-300 rounded-md flex items-center justify-center";

  return (
    <div className="flex flex-col min-w-screen min-h-screen overflow-hidden">
      <div id="settingsBar" className="bg-base-200 flex flex-row w-full p-3 gap-3 items-center">

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

        <div className="flex-grow"/>
        <div title="Pretty cool project" className="text-lg font-bold w-fit text-center">VRC Haptics Manager</div>
        <div className="flex-grow" />

        <Link title="Settings" className={linkClass} to="/settings">
          <AiOutlineSetting
            className={clsx(defaultClass, {//if we are in settings, apply selected class
              [selectedClass]: location.pathname === "/settings",
            })}
          />
        </Link>
        
      </div>
      <div id="windowContainer" className="flex flex-1 m-4">
        <Outlet />
      </div>
    </div>
  );
}
