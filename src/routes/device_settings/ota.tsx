import { useState, useEffect, useMemo, useCallback } from "react";
import { error, trace } from "@tauri-apps/plugin-log";
import { fetch } from "@tauri-apps/plugin-http";
import { useDeviceContext } from "../../context/DevicesContext";
import { useSettingsContext } from "../../context/SettingsProvider";
import { DEFAULT_REPO } from "../../context/SettingsProvider";
import { FaArrowRight } from "react-icons/fa";
import {
  commands,
  type GitRepo,
  type ESP32Model,
} from "../../bindings";

const LATEST_TAG = "latest";

interface Release {
  pre_release: boolean;
  tag_name: string;
  git_url: string;
  assets: ReleaseAsset[];
}

interface ReleaseAsset {
  file_name: string;
  download_url: string;
  download_count: number;
}

interface EligibleDevice {
  id: string;
  name: string;
  esp: ESP32Model;
}

function espToAssetSuffix(esp: ESP32Model): string {
  return esp.toLowerCase().replace(/-/g, "");
}

function releaseMatchesEsp(release: Release, esp: ESP32Model): boolean {
  const key = espToAssetSuffix(esp);
  return release.assets.some(
    (a) => a.file_name.endsWith(".bin") && a.file_name.includes(key),
  );
}

function pickAsset(
  release: Release,
  esp: ESP32Model,
): ReleaseAsset | undefined {
  const key = espToAssetSuffix(esp);
  return release.assets.find(
    (a) => a.file_name.endsWith(".bin") && a.file_name.includes(key),
  );
}

function findLatest(releases: Release[]): Release | undefined {
  return releases.find((r) => !r.pre_release) ?? releases[0];
}

async function fetchGithubReleases(repo: GitRepo): Promise<Release[]> {
  const url = `https://api.github.com/repos/${repo.owner}/${repo.name}/releases`;
  trace(`[OTA] fetch START ${url}`);

  let response: Response;
  try {
    response = await fetch(url);
  } catch (e) {
    error(`[OTA] fetch threw: ${e}`);
    throw e;
  }

  trace(`[OTA] fetch responded ${response.status}`);

  if (!response.ok) {
    const body = await response.text().catch(() => "");
    throw new Error(`GitHub API ${response.status}: ${body}`.trim());
  }

  const text = await response.text();
  trace(`[OTA] response body length: ${text.length}`);
  const raw: Array<Record<string, any>> = JSON.parse(text);
  trace(`[OTA] parsed ${raw.length} releases`);

  return raw.map(
    (item): Release => ({
      tag_name: item.tag_name,
      git_url: item.html_url,
      pre_release: item.prerelease,
      assets: (item.assets as Array<Record<string, any>>).map(
        (asset): ReleaseAsset => ({
          file_name: asset.name,
          download_url: asset.browser_download_url,
          download_count: asset.download_count,
        }),
      ),
    }),
  );
}

export default function OtaUpdate(): JSX.Element {
  const { repositories } = useSettingsContext();
  const { devices } = useDeviceContext();

  const [eligibleDevices, setEligibleDevices] = useState<EligibleDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string>("");
  const [releaseTag, setReleaseTag] = useState<string>(LATEST_TAG);
  const [repository, setRepository] = useState<GitRepo>(DEFAULT_REPO);
  const [availableReleases, setAvailableReleases] = useState<Release[]>([]);
  const [releasesLoading, setReleasesLoading] = useState(false);
  const [releasesError, setReleasesError] = useState<string>("");
  const [isUpdating, setIsUpdating] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<string>("");

  // ── Resolve eligible devices (debounced by serializing to string) ─────
  const deviceMacs = devices
    .filter((d): d is typeof d & { variant: "Wifi" } => d.variant === "Wifi")
    .map((d) => d.value.mac)
    .join(",");

  useEffect(() => {
    let cancelled = false;

    async function resolve() {
      const wifiDevices = devices.filter(
        (d): d is typeof d & { variant: "Wifi" } => d.variant === "Wifi",
      );

      const results = await Promise.allSettled(
        wifiDevices.map(async (device) => {
          const res = await commands.getDeviceEspModel(device.value.mac);
          if (res.status === "ok") return { device, esp: res.data };
          throw new Error(String(res.error));
        }),
      );

      if (cancelled) return;

      const eligible: EligibleDevice[] = results
        .filter(
          (
            r,
          ): r is PromiseFulfilledResult<{
            device: (typeof wifiDevices)[0];
            esp: ESP32Model;
          }> => r.status === "fulfilled",
        )
        .map((r) => ({
          id: r.value.device.value.mac,
          name: r.value.device.value.name,
          esp: r.value.esp,
        }));

      setEligibleDevices((prev) => {
        const key = (list: EligibleDevice[]) =>
          list.map((d) => `${d.id}:${d.esp}`).join(",");
        return key(prev) === key(eligible) ? prev : eligible;
      });
    }

    resolve();
    return () => {
      cancelled = true;
    };
  }, [deviceMacs]);

  // ── Fetch releases when repository changes ────────────────────────────
  const repoKey = `${repository.owner}/${repository.name}`;

  useEffect(() => {
    let stale = false;

    setAvailableReleases([]);
    setReleasesError("");
    setReleasesLoading(true);
    setReleaseTag(LATEST_TAG);

    fetchGithubReleases(repository)
      .then((releases) => {
        if (stale) return;
        const allAssets = releases.flatMap((r) =>
          r.assets.map((a) => a.file_name),
        );
        trace(
          `[OTA] Stored ${releases.length} releases. Assets: ${allAssets.join(", ")}`,
        );
        setAvailableReleases(releases);
      })
      .catch((err: unknown) => {
        if (stale) return;
        const msg = err instanceof Error ? err.message : String(err);
        error(`[OTA] Release fetch failed: ${msg}`);
        setReleasesError(msg);
      })
      .finally(() => {
        setReleasesLoading(false);
      });

    return () => {
      stale = true;
    };
  }, [repoKey]); // stable string, not object reference

  // ── Derived data ──────────────────────────────────────────────────────
  const selectedEsp = useMemo(
    () => eligibleDevices.find((d) => d.id === selectedDevice),
    [eligibleDevices, selectedDevice],
  );

  const filteredReleases = useMemo(() => {
    if (!selectedEsp || availableReleases.length === 0) return [];
    const filtered = availableReleases.filter((r) =>
      releaseMatchesEsp(r, selectedEsp.esp),
    );
    trace(
      `[OTA] Filtered ${filtered.length}/${availableReleases.length} for ${selectedEsp.esp}`,
    );
    return filtered;
  }, [selectedEsp, availableReleases]);

  // ── Update handler ────────────────────────────────────────────────────
  const handleUpdate = useCallback(async () => {
    if (!selectedEsp) {
      setUpdateStatus("✗ No device selected");
      return;
    }

    setIsUpdating(true);
    setUpdateStatus("Resolving release…");

    try {
      const release =
        releaseTag === LATEST_TAG
          ? findLatest(filteredReleases)
          : filteredReleases.find((r) => r.tag_name === releaseTag);

      if (!release) {
        setUpdateStatus(`✗ No compatible release for "${releaseTag}"`);
        return;
      }

      const asset = pickAsset(release, selectedEsp.esp);
      if (!asset) {
        setUpdateStatus(
          `✗ ${release.tag_name} has no binary for ${selectedEsp.esp}`,
        );
        return;
      }

      setUpdateStatus(`Downloading ${asset.file_name}…`);
      trace(`[OTA] Downloading ${asset.download_url}`);

      const response = await fetch(asset.download_url, { method: "GET" });
      if (!response.ok) {
        throw new Error(`Download failed: HTTP ${response.status}`);
      }

      const arrayBuffer = await response.arrayBuffer();
      if (arrayBuffer.byteLength === 0)
        throw new Error("Downloaded firmware is empty");
      trace(`[OTA] Downloaded ${arrayBuffer.byteLength} bytes`);

      const bytes: number[] = Array.from(new Uint8Array(arrayBuffer));

      setUpdateStatus("Flashing firmware…");
      const res = await commands.startDeviceUpdate({
        id: selectedDevice,
        method: { OTA: "Haptics-OTA" },
        bytes,
      });
      if (res.status === "error") throw new Error(res.error);

      setUpdateStatus("✓ Update complete");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      error(`[OTA] Update failed: ${msg}`);
      setUpdateStatus(`✗ ${msg}`);
    } finally {
      setIsUpdating(false);
    }
  }, [selectedEsp, selectedDevice, releaseTag, filteredReleases]);

  // ── Render ────────────────────────────────────────────────────────────
  const versionLabel = !selectedDevice
    ? "Select Version"
    : releasesLoading
      ? "Loading…"
      : releasesError
        ? "Error"
        : `Release: ${releaseTag}`;

  return (
    <div id="OtaModule" className="flex min-w-full max-h-min items-center">
      <div className="dropdown dropdown-start">
        <div tabIndex={0} role="button" className="btn m-1">
          {selectedEsp
            ? `${selectedEsp.name} : ${selectedEsp.esp}`
            : "Select Device"}
        </div>
        <ul
          tabIndex={0}
          className="dropdown-content menu bg-base-200 rounded-box z-[1] w-52 p-2 shadow"
        >
          {eligibleDevices.length === 0 && (
            <li className="p-2 text-center">No Devices Connected</li>
          )}
          {eligibleDevices.map((d) => (
            <li key={d.id}>
              <a onClick={() => setSelectedDevice(d.id)}>
                {d.name} : {d.esp}
              </a>
            </li>
          ))}
        </ul>
      </div>

      <FaArrowRight />

      <div className="dropdown dropdown-start">
        <div tabIndex={0} role="button" className="btn m-1">
          {repository.owner}/{repository.name}
        </div>
        <ul
          tabIndex={0}
          className="dropdown-content menu bg-base-200 rounded-box z-[1] p-2 shadow"
        >
          {repositories.map((repo: GitRepo, i: number) => (
            <li key={i}>
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

      <div className="dropdown dropdown-start">
        <div tabIndex={0} role="button" className="btn m-1">
          {versionLabel}
        </div>
        <ul
          tabIndex={0}
          className="dropdown-content menu bg-base-200 rounded-box z-[1] w-52 p-2 shadow"
        >
          {!selectedDevice ? (
            <li className="text-center p-2">Please select a device</li>
          ) : releasesLoading ? (
            <li className="text-center p-2">
              <span className="loading loading-spinner loading-sm" /> Loading…
            </li>
          ) : releasesError ? (
            <li className="text-center p-2 text-error">{releasesError}</li>
          ) : filteredReleases.length === 0 ? (
            <li className="text-center p-2">No compatible releases</li>
          ) : (
            filteredReleases.map((release) => (
              <li key={release.tag_name}>
                <a onClick={() => setReleaseTag(release.tag_name)}>
                  {release.tag_name}
                  {release.pre_release && " Pre-Release"}
                  {!release.pre_release &&
                    release === findLatest(filteredReleases) && (
                      <span className="badge badge-primary badge-sm">
                        Latest
                      </span>
                    )}
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
            <span className="loading loading-spinner" /> Updating…
          </>
        ) : (
          "Upload Firmware"
        )}
      </button>

      {updateStatus && (
        <div
          className={`ml-2 ${updateStatus.startsWith("✓") ? "text-success" : updateStatus.startsWith("✗") ? "text-error" : "text-info"}`}
        >
          {updateStatus}
        </div>
      )}
    </div>
  );
}
