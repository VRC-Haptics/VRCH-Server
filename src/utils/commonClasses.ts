let Titles = "text-2xl font-bold padding-5 text-center";


export const defaultDevice = {MAC:"", IP:"", DisplayName: "", Port: 0, TTL:0}
export interface Device {
    MAC: string;
    IP: string,
    DisplayName:string,
    Port: number,
    TTL: number,
}

export const defaultAvatar = {avatar_id: "", menu_parameters: [], haptic_parameters: []};
export interface avatar {
    avatar_id: string,
    menu_parameters: string[],
    haptic_parameters: string[],
}



export const defaultVrcInfo = {in_port: 0, out_port:0, avatar:defaultAvatar};
export interface vrcInfo {
    in_port: number,
    out_port: number,
    avatar: avatar,
}

export default Titles;