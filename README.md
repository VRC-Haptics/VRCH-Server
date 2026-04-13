# VRC Haptics

A GUI interface for the haptic server.

# Usage

Simply launch the manager when starting VRC or other games, devices will auto connect and the games will auto configure themselves.

## First Time setup

Grab either of the installers from the releases page and click through it.

Everything should configure itself and auto connect if you have either a bhaptics device closeby or a native vrch device on the same wifi network.

When starting vrc, the red dot on the **VRC** page on the right side of the screen will turn green when it is connected to a vrc instance.

To connect to a Quest standalone VRC instance the computer running this manager must be connected to the same WIFI network. Running the manager on quest natively is currently not supported.

# Development

## setup:
#### Development
- `pnpm i` -> Installs dependencies (both rust and node)
- `pnpm run tauri dev` -> Start the dev server. 

#### Build:
- `pnpm i` -> Installs dependencies (both rust and node)
- `pnpm run tauri build` -> Builds installer under: `./src-tauri/target/release/bundle/<some_subfolder>`

#### Sidecars:
This project has a few sidecars
 - Windows Registry Editor: `./src-elevated-register`
 - Game Proxy: `./src-proxy`
 - MDNS Listener: `./src-vrc-oscquery/listen-for-vrc`

This is a project VERY early in its development so reporting issues and making contributions (even if they are small) is much appreciated.

## TODO's:

### Backend:

 - Add game support
 - Support more BLE devices (only x16 vests are supported)

### Frontend:

 - Reimplement OTA updates
 - Implement device settings editor
 - Implement Serial device updates

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

(I used Jetbrains Rider for the C# sidecars)