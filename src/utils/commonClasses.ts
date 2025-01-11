let Titles = "text-2xl font-bold padding-5 text-center";

export const defaultDevice = {
  MAC: "",
  IP: "",
  DisplayName: "",
  Port: 0,
  TTL: 0,
};
export interface Device {
  MAC: string;
  IP: string;
  DisplayName: string;
  Port: number;
  TTL: number;
}

export interface Avatar {
  avatar_id: string;
  menu_parameters?: oscPair[];
  haptic_parameters?: oscPair[];
}

export interface oscPair {
    address: string,
    value: string,
}

export interface VrcInfo {
  osc_server?: any; // Assuming you don't need to use this directly
  in_port?: number;
  out_port?: number;
  avatar?: Avatar;
  raw_parameters: Record<string, string>;
}

export const defaultVrcInfo: VrcInfo = {
  in_port: 0,
  out_port: 0,
  avatar: { avatar_id: "", menu_parameters: [], haptic_parameters: [] },
  raw_parameters: {},
};

export default Titles;
