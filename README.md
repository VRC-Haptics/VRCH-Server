# VRC Haptics Headless

THIS IS THE HEAD-LESS VERSION OF VRCH. IT IS CLI ONLY. FOR THE FULL APP GO HERE:
[MAIN branch](https://github.com/virtuallyaverage/vrch-gui)

A GUI interface for providing osc haptics from Vrchat.

# Usage

After the first setup it should be simple plug and play with no need for user configuration on each startup.

## First Time setup

Grab the installer from the releases page and it should install on windows.

- _MDNS sidecars are the only thing I can think of that would stop it working on other platforms, since they are precomiled executables. If you have a solution to this it would be much appreciated!_

Once installed, it should be as simple as opening the application and assigning which parameters you want your device tied to. Here are the basics:

- All parameters must have a base of `/h`, parameters without this prefix are not currently tracked by the program.
- Since it is possible to have a large number of haptic nodes, the parameters are divided up into groups, and are formatted like so: Group{`Front@0:15`}; `/h/Front_0, /h/Front_1, ...., /h/Front_15`
  - Singular nodes can be represented by: Group{`Front@0:0`}

# Development

## setup:
#### Development
- `pnpm i` -> Install dependencies (both rust and node)
- `pnpm run tauri dev` -> Start the dev server. 

#### Build:
- `pnpm i` -> Install dependencies (both rust and node)
- `pnpm run tauri build` -> Builds installer under: `./src-tauri/target/release/bundle/<some_subfolder>`

#### Sidecars:
This project has two C# sidecars, for mdns discovery (since rust sucks with it). 
The source code for them can be found here:
 - VRC: [oyasumivr_oscquery](https://github.com/Raphiiko/oyasumivr_oscquery/tree/main/src-mdns-sidecar)
 - Haptics: [basic-mdns-cli](https://github.com/virtuallyaverage/basic-mdns-cli)

This is a project VERY early in its development so reporting issues and making contributions (even if they are small) is much appreciated.

## TODO's:

### Backend:
- Implement: VRC OSC client
    - Currently only can recieve messages from VRC
- Implement: VRC radial configuration
  - Intensity Scaling (less BRRR).
  - Intensity Offset (mimic smaller contacts).
  - Use Tauri Store to save previous settings.
  - restore settings across avatars.

### Frontend:

- Fix: Group editor
  - Complete rework or small edits is fine
  - Show generated addresses for debugging
- Fix: scaling across different DPI windows
- Refactor: Context providers
    - I am new to react and know the way I set them up is not optimal
- Implement: Game Settings
  - Show number of parameters found on avatar (like vrcft)
  - show raw paramers
  - settings like intensity
- Working 

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

(I used Jetbrains Rider for the C# sidecars)