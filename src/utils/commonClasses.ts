let Titles = "text-2xl font-bold padding-5 text-center";

export const defaultDevice = {
  mac: "",
  ip: "",
  display_name: "",
  port: 0,
  ttl: 0,
  addr_groups: [],
  num_motors: 0,
  sens_mult: 1.0,
};

export interface Device {
  mac: string;
  ip: string;
  display_name: string;
  port: number;
  ttl: number;
  addr_groups: AddressGroup[];
  num_motors: number;
  sens_mult: number;
}

export const defaultAddressGroup = {
  name: "",
  start: 0,
  end: 0,
}

export interface AddressGroup {
  name: string;
  start: number;
  end: number;
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
