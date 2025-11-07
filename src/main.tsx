import React from "react";
import ReactDOM from "react-dom/client";
import { createBrowserRouter, RouterProvider } from "react-router-dom";
import Layout from "./layout";
import ErrorPage from "./error-page";
import Home from "./routes/home";
import Settings from "./routes/settings";
import { TauriProvider } from "./context/TauriProvider";
import "./styles.css";
import { SettingsProvider } from "./context/SettingsProvider";
import GameSettings from "./routes/game_settings/game_settings";
import GlobalMapContainer from "./routes/haptic_map";
import DeviceSettings from "./routes/device_settings";

const router = createBrowserRouter([
  {
    path: "/",
    element: <Layout />,
    errorElement: <ErrorPage />,
    children: [
      {
        index: true,
        element: <Home />,
      },
      {
        path: "/settings",
        element: <Settings />,
      },
      {
        path: "/game_settings",
        element: <GameSettings />,
      },
      {
        path: "/global_map",
        element: <GlobalMapContainer />,
      },
      {
        path: "/device_settings",
        element: <DeviceSettings/>
      }
    ],
  },
]);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <TauriProvider>
      <SettingsProvider>
          <RouterProvider router={router} />
      </SettingsProvider>
    </TauriProvider>
  </React.StrictMode>
);
