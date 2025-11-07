import { useState, useEffect } from "react";
import { error, trace } from "@tauri-apps/plugin-log";
import { invoke } from "@tauri-apps/api/core";
import { useDeviceContext } from "../../context/DevicesContext";
import { FaArrowRight } from "react-icons/fa";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { GitRepo } from "../../utils/commonClasses";
import { useSettingsContext } from "../../context/SettingsProvider";
import { DEFAULT_REPO } from "../../context/SettingsProvider";

const LATEST_TAG: string = "latest";

enum OtaState {
  NotStarted,
  DownloadingFw,
  PushingUpdate,
  Restarting,
}

async function githubReleases(repo: GitRepo): Promise<Release[]> {
  const response = await fetch(
    `https://api.github.com/repos/${repo.owner}/${repo.name}/releases`
  );

  if (!response.ok) {
    error(`GitHub API error: ${response.status}`);
    throw new Error(`GitHub API error: ${response.status}`);
  }

  const releases: Release[] = [];
  const resp_releases = await response.json();
  resp_releases.forEach((item: any, idx:number) => {
    const assets: ReleaseAsset[] = item.assets.map(
      (asset: any) =>
        new ReleaseAsset(
          asset.name,
          asset.browser_download_url,
          asset.download_count
        )
    );

    releases.push(
      new Release(idx === 0, item.tag_name, item.html_url, item.prerelease, assets)
    );
  });

  return releases;
}

class Release {
  is_latest: boolean = true;
  pre_release: boolean = true;
  tag_name: string = "";
  git_url: string = "";
  assets: ReleaseAsset[] = [];

  constructor(
    is_latest: boolean,
    tag_name: string,
    git_url: string,
    pre_release: boolean,
    assets: ReleaseAsset[]
  ) {
    this.is_latest = is_latest;
    this.pre_release = pre_release;
    this.tag_name = tag_name;
    this.git_url = git_url;
    this.assets = assets;
  }
}

class ReleaseAsset {
  file_name: string;
  download_url: string;
  download_count: number;

  constructor(file_name: string, download_url: string, download_count: number) {
    this.file_name = file_name;
    this.download_url = download_url;
    this.download_count = download_count;
  }
}

export default function OtaUpdate() {
  const { repositories } = useSettingsContext();
  const { devices } = useDeviceContext();
  // all devices elligable for OTA updates.
  const [eligibleDevices, setEligibleDevices] = useState<
    { id: string; name: string; esp: string }[]
  >([]);
  // the device id currently focused on for fw updates.
  const [selectedDevice, setSelectedDevice] = useState<string>("");
  // the currently selected release tag.
  const [releaseTag, setReleaseTag] = useState<string>(LATEST_TAG);
  // where we should pull releases from
  const [repository, setRepository] = useState<GitRepo>(DEFAULT_REPO);
  // releases that are in tthe currently selected repository
  const [availableReleases, setAvailableReleases] = useState<Release[]>([]);
  // releases that are compatible with our current device.
  const [filteredReleases, setFilteredReleases] = useState<Release[]>([]);
  const [isUpdating, setIsUpdating] = useState(false);
  // update status that is shown on the bottom of the panel.
  const [updateStatus, setUpdateStatus] = useState<string>("");

  /// get list of devices that we can send updates to.
  useEffect(() => {
    async function filterDevices() {
      const checked = await Promise.all(
        devices.map(async (device) => {
          try {
            const model = await invoke("get_device_esp_model", {
              id: device.id,
            });
            return model !== "Unknown" ? { ...device, espModel: model } : null;
          } catch {
            return null;
          }
        })
      );
      let elig_dev = checked
        .filter((d) => d !== null)
        .map((d) => ({
          id: d.id,
          name: d.name,
          esp: d.espModel as string,
        }));
      setEligibleDevices(elig_dev);
    }

    filterDevices();
  }, [devices]);

  /// Refresh available tags when github configuration changes.
  useEffect(() => {
    const refreshTags = async () => {
      setAvailableReleases(await githubReleases(repository));
    };
    refreshTags();
  }, [repository]);

  /// refresh filtered releases when the full releases update
  useEffect(() => {
    const device = eligibleDevices.find((d) => d.id === selectedDevice);
    if (device == null) {
      error("Device to udpate is null");
      return;
    }

    const platform = device.esp;
    const filter = `${platform.toLowerCase()}.bin`;
    const filtered = availableReleases.filter((release) =>
      release.assets.some(
        (asset) => asset.file_name.endsWith(filter)
      )
    );
    setFilteredReleases(filtered);
  }, [selectedDevice, availableReleases]);

  /// Handle actually updating button.
  const handleUpdate = async () => {
    if (!selectedDevice || !releaseTag) {
      setUpdateStatus("Select device and tag");
      return;
    }

    setIsUpdating(true);
    setUpdateStatus("");

    try {
      // Target release
      const device = eligibleDevices.find((d) => d.id === selectedDevice);
      let release: Release | undefined = undefined;
      if (releaseTag === LATEST_TAG) {
        release = filteredReleases.find((r) => r.is_latest);
        // sort filtered releases by 
      } else {
        release = filteredReleases.find((r) => r.tag_name === releaseTag);
      }
 
      if (release == null || device == null) {
        error(`Unable to find release with tag: ${releaseTag}`);
        return;
      }

      // find specific binary
      const filter = `${device.esp.toLowerCase()}.bin`;
      const bin_asset = release.assets.find((d) =>
        d.file_name.endsWith(filter)
      );
      if (bin_asset == null) {
        error(
          `Unable to find asset with filter: ${filter} and release: ${releaseTag}`
        );
        return;
      }

      trace("URL: " + bin_asset.download_url);
      const response = await tauriFetch(bin_asset.download_url, {
        method: "GET",
      });
      if (!response.ok)
        throw new Error(`Failed to fetch firmware: ${response.status}`);

      const arrayBuffer = await response.arrayBuffer();
      const bytes = Array.from(new Uint8Array(arrayBuffer));

      // Invoke Tauri command
      await invoke("start_device_update", {
        fw: {
          id: selectedDevice,
          method: { OTA: "Haptics-OTA" },
          bytes: bytes,
        },
      });

      setUpdateStatus("✓ Update complete");
    } catch (err) {
      error(`Update failed: ${err}`);
      setUpdateStatus(
        `✗ Error: ${err instanceof Error ? err.message : String(err)}`
      );
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <div id="OtaModule" className="flex min-w-full max-h-min items-center">
      <div id="deviceSelector" className="dropdown dropdown-start">
        <div tabIndex={0} role="button" className="btn m-1">
          {selectedDevice
            ? `${
                eligibleDevices.find((d) => d.id === selectedDevice)?.name
              } : ${eligibleDevices.find((d) => d.id === selectedDevice)?.esp}`
            : "Select Device"}
        </div>
        <ul
          tabIndex={0}
          className="dropdown-content menu bg-base-200 rounded-box z-[1] w-52 p-2 shadow"
        >
          {eligibleDevices.map((device) => (
            <li key={device.id}>
              <a onClick={() => setSelectedDevice(device.id)}>
                {device.name} : {device.esp}
              </a>
            </li>
          ))}
        </ul>
      </div>
      <FaArrowRight />
      <div id="repositorySelector" className="dropdown dropdown-start">
        <div tabIndex={0} role="button" className="btn m-1">
          {repository
            ? `${repository.owner}/${repository.name}`
            : "Select Repository"}
        </div>
        <ul
          tabIndex={0}
          className="dropdown-content menu bg-base-200 rounded-box z-[1] p-2 shadow"
        >
          {repositories.map((repo, index) => (
            <li key={index}>
              <a onClick={() => setRepository(repo)}>
                {repo.owner}/{repo.name}
                {repo.owner === DEFAULT_REPO.owner &&
                  repo.name === DEFAULT_REPO.name && (
                    <span className="badge badge-primary badge-sm">
                      Official
                    </span>
                  )}
              </a>
            </li>
          ))}
        </ul>
      </div>
      <FaArrowRight />
      <div id="versSelector" className="dropdown dropdown-start">
        <div tabIndex={0} role="button" className="btn m-1">
          {releaseTag ? `Release: ${releaseTag}` : "Select Version"}
        </div>
        <ul
          tabIndex={0}
          className="dropdown-content menu bg-base-200 rounded-box z-[1] w-52 p-2 shadow"
        >
          {availableReleases.length === 0 ? (
            <li className="text-center p-2">Please select a device</li>
          ) : filteredReleases.length === 0 ? (
            <li className="text-center p-2">
              No versions for your device in this repository
            </li>
          ) : (
            filteredReleases.map((release) => (
              <li key={release.tag_name}>
                <a onClick={() => setReleaseTag(release.tag_name)}>
                  {release.tag_name} {release.pre_release ? "Pre-Release" : ""} {release.is_latest ? (
                    <span className="badge badge-primary badge-sm">
                      Latest
                    </span>
                  ) : ""}
                </a>
              </li>
            ))
          )}
        </ul>
      </div>
      <FaArrowRight />
      <button
        className="btn"
        onClick={handleUpdate}
        disabled={isUpdating || !selectedDevice}
      >
        {isUpdating ? (
          <>
            <span className="loading loading-spinner"></span>
            Updating...
          </>
        ) : (
          "Upload Firmware"
        )}
      </button>
      {updateStatus && (
        <div
          className={`mt-2 ${
            updateStatus.startsWith("✓") ? "text-success" : "text-error"
          }`}
        >
          {updateStatus}
        </div>
      )}
    </div>
  );
}
