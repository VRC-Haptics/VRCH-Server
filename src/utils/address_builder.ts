export const addressBuilder = (
    group_name: string,
    index: number,
) => {
    const prefix = "/avatar/parameters/h/";
    return prefix.concat(group_name, "_", index.toString());
}