import type { Config } from "@netlify/edge-functions";

const RELEASES_API = "https://api.github.com/repos/percyho/CapsuleMeter/releases?per_page=100";
const RELEASE_ASSET_PREFIX = "/percyho/CapsuleMeter/releases/download/";

type ReleaseAsset = {
  name?: string;
  size?: number;
  browser_download_url?: string;
};

type GitHubRelease = {
  draft?: boolean;
  prerelease?: boolean;
  published_at?: string | null;
  created_at?: string;
  assets?: ReleaseAsset[];
};

type Platform = "windows" | "mac";

function stableReleases(releases: unknown): GitHubRelease[] {
  return (Array.isArray(releases) ? releases : [])
    .filter((release): release is GitHubRelease =>
      !!release && !release.draft && !release.prerelease && Array.isArray(release.assets)
    )
    .sort((a, b) =>
      Date.parse(b.published_at || b.created_at || "") - Date.parse(a.published_at || a.created_at || "")
    );
}

function findWindowsInstaller(releases: unknown): ReleaseAsset | undefined {
  for (const release of stableReleases(releases)) {
    const installer = release.assets?.find((asset) =>
      typeof asset.name === "string" &&
      asset.name.toLowerCase().endsWith("_x64-setup.exe") &&
      typeof asset.browser_download_url === "string"
    );
    if (installer) return installer;
  }
  return undefined;
}

function isMacInstaller(asset: ReleaseAsset): boolean {
  if (typeof asset.name !== "string" || typeof asset.browser_download_url !== "string") return false;
  const name = asset.name.toLowerCase();
  return /\.(dmg|pkg)$/i.test(name) || /\.app\.tar\.gz$/i.test(name) ||
    (/\.zip$/i.test(name) && /mac|macos|osx|darwin|arm64|aarch64|x64|x86_64|universal/i.test(name));
}

function macArchitecture(asset: ReleaseAsset): "universal" | "arm" | "x64" | "any" {
  const name = (asset.name || "").toLowerCase();
  if (/universal2?|universal-/.test(name)) return "universal";
  if (/aarch64|arm64/.test(name)) return "arm";
  if (/x86_64|x64|amd64/.test(name)) return "x64";
  return "any";
}

function findMacInstaller(releases: unknown, architecture: string): ReleaseAsset | undefined {
  for (const release of stableReleases(releases)) {
    const assets = (release.assets || []).filter(isMacInstaller).sort((a, b) => {
      const rank = (asset: ReleaseAsset) => {
        const name = (asset.name || "").toLowerCase();
        return name.endsWith(".dmg") ? 0 : name.endsWith(".pkg") ? 1 : name.endsWith(".zip") ? 2 : 3;
      };
      return rank(a) - rank(b);
    });
    const universal = assets.find((asset) => macArchitecture(asset) === "universal");
    const matching = architecture !== "unknown"
      ? assets.find((asset) => macArchitecture(asset) === architecture)
      : undefined;
    const generic = assets.find((asset) => macArchitecture(asset) === "any");
    if (universal || matching || generic) return universal || matching || generic;
  }
  return undefined;
}

function getDownloadUrl(installer?: ReleaseAsset): URL | undefined {
  if (typeof installer?.browser_download_url !== "string") return undefined;
  try {
    const url = new URL(installer.browser_download_url);
    return url.protocol === "https:" && url.hostname === "github.com" && url.pathname.startsWith(RELEASE_ASSET_PREFIX)
      ? url
      : undefined;
  } catch {
    return undefined;
  }
}

function errorResponse(platform: Platform, message: string): Response {
  return new Response(message, {
    status: platform === "mac" ? 404 : 503,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Expose-Headers": "Content-Length, Content-Disposition, Content-Type",
      "Cache-Control": "no-store",
      "Content-Type": "text/plain; charset=utf-8",
    },
  });
}

export default async function latestInstallerDownload(request: Request): Promise<Response> {
  const requestUrl = new URL(request.url);
  const platform: Platform = requestUrl.searchParams.get("platform") === "mac" ? "mac" : "windows";
  const architecture = requestUrl.searchParams.get("arch") || "unknown";
  const streamForProgress = requestUrl.searchParams.get("progress") === "1";

  try {
    const releasesResponse = await fetch(RELEASES_API, {
      headers: {
        accept: "application/vnd.github+json",
        "user-agent": "CapsuleMeter-Website",
        "x-github-api-version": "2022-11-28",
      },
    });
    if (!releasesResponse.ok) {
      return errorResponse(platform, "Could not load a compatible installer from the latest releases.");
    }

    const releases: unknown = await releasesResponse.json();
    const installer = platform === "mac"
      ? findMacInstaller(releases, architecture)
      : findWindowsInstaller(releases);
    const downloadUrl = getDownloadUrl(installer);
    if (!installer?.name || !downloadUrl) {
      return errorResponse(platform, platform === "mac"
        ? "No compatible macOS installer is available."
        : "No Windows installer is available in the latest releases.");
    }

    if (platform === "windows" && !streamForProgress) {
      return new Response(null, {
        status: 302,
        headers: { location: downloadUrl.href, "Cache-Control": "no-store" },
      });
    }

    const assetResponse = await fetch(downloadUrl.href, {
      headers: { accept: "application/octet-stream" },
    });
    if (!assetResponse.ok || !assetResponse.body) {
      return errorResponse(platform, "Could not retrieve the selected installer.");
    }

    const safeFilename = installer.name.replace(/[\\/:*?"<>|\r\n]/g, "_");
    const headers = new Headers({
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Expose-Headers": "Content-Length, Content-Disposition, Content-Type",
      "Cache-Control": "no-store",
      "Content-Type": "application/octet-stream",
      "Content-Disposition": `attachment; filename="${safeFilename}"; filename*=UTF-8''${encodeURIComponent(safeFilename)}`,
      "X-Content-Type-Options": "nosniff",
    });
    const contentLength = assetResponse.headers.get("content-length") || installer.size;
    if (contentLength) headers.set("Content-Length", String(contentLength));

    return new Response(assetResponse.body, { status: 200, headers });
  } catch {
    return errorResponse(platform, "The installer service is temporarily unavailable.");
  }
}

export const config: Config = {
  path: "/download",
  method: "GET",
  cache: "manual",
  onError: "bypass",
};
