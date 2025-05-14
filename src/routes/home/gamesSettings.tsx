import { useVrcContext } from "../../context/VrcContext";
import { VrcInfo } from "../../utils/vrc_info_classes";
import clsx from "clsx";

interface VrcInfoCardProps {
  /** The VrcInfo object fetched from the backend */
  vrcInfo: VrcInfo;
}

/**
 * Displays a snapshot of the currently‑connected VRChat state.
 *
 * Tailwind‑powered, rounded‑corner card with soft shadow.
 */
export default function VrcInfoCard({}) {
  const  {vrcInfo} = useVrcContext();

  return (
    <div className="flex-shrink-0 flex-col min-w-0 bg-base-200 rounded-md p-2 space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">VRChat Connection</h2>
        <span
          className={clsx(
            "h-3 w-3 rounded-full",
            vrcInfo.vrc_connected ? "bg-emerald-500" : "bg-rose-500"
          )}
          title={vrcInfo.vrc_connected ? "Connected" : "Disconnected"}
        />
      </div>

      {/* Connection details */}
      <div className="mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm md:text-base">
        <span className="font-medium">In&nbsp;Port:</span>
        <span>{vrcInfo.in_port ?? "—"}</span>
        <span className="font-medium">Out&nbsp;Port:</span>
        <span>{vrcInfo.out_port ?? "—"}</span>
        <span className="font-medium">Cache&nbsp;Length:</span>
        <span>{vrcInfo.cache_length}</span>
      </div>

      {/* Avatar section */}
      {vrcInfo.avatar && (
        <div className="mt-6">
          <h3 className="text-lg font-semibold">Avatar</h3>
          <div className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm md:text-base">
            <span className="font-medium">ID:</span>
            <span>{vrcInfo.avatar.id}</span>
            {vrcInfo.avatar.prefab_name && (
              <>
                <span className="font-medium">Prefab:</span>
                <span>{vrcInfo.avatar.prefab_name}</span>
              </>
            )}
            {vrcInfo.avatar.conf && (
              <>
                <span className="font-medium">Map&nbsp;Name:</span>
                <span>{vrcInfo.avatar.conf.meta.map_name}</span>
                <span className="font-medium">Version:</span>
                <span>{vrcInfo.avatar.conf.meta.map_version}</span>
                <span className="font-medium">Author:</span>
                <span>{vrcInfo.avatar.conf.meta.map_author}</span>
                <span className="font-medium">Haptic&nbsp;Nodes:</span>
                <span>{vrcInfo.avatar.conf.nodes.length}</span>
              </>
            )}
          </div>
        </div>
      )}

      {/* OSC summary */}
      <div className="mt-6">
        <h3 className="text-lg font-semibold">OSC</h3>
        <div className="mt-2 text-sm md:text-base">
          <p>
            <span className="font-medium">Available&nbsp;Parameters:</span>{" "}
            {Object.keys(vrcInfo.available_parameters).length}
          </p>
          <p>
            <span className="font-medium">Cached&nbsp;Parameters:</span>{" "}
            {Object.keys(vrcInfo.parameter_cache).length}
          </p>
        </div>
      </div>
    </div>
  );
}