const { Readable } = require("node:stream");

const RELEASES_API = "https://api.github.com/repos/percyho/CapsuleMeter/releases?per_page=100";
const RELEASE_ASSET_PREFIX = "/percyho/CapsuleMeter/releases/download/";

function stableReleases(releases) {
  return (Array.isArray(releases) ? releases : [])
    .filter((release) => release && !release.draft && !release.prerelease && Array.isArray(release.assets))
    .sort((a, b) => Date.parse(b.published_at || b.created_at || 0) - Date.parse(a.published_at || a.created_at || 0));
}

function findWindowsInstaller(releases) {
  for (const release of stableReleases(releases)) {
    const installer = release.assets.find((asset) =>
      typeof asset.name === "string" &&
      asset.name.toLowerCase().endsWith("_x64-setup.exe") &&
      typeof asset.browser_download_url === "string"
    );
    if (installer) return installer;
  }
  return null;
}

function isMacInstaller(asset) {
  if (!asset || typeof asset.name !== "string" || typeof asset.browser_download_url !== "string") return false;
  const name = asset.name.toLowerCase();
  return /\.(dmg|pkg)$/i.test(name) || /\.app\.tar\.gz$/i.test(name) ||
    (/\.zip$/i.test(name) && /mac|macos|osx|darwin|arm64|aarch64|x64|x86_64|universal/i.test(name));
}

function macArchitecture(asset) {
  const name = asset.name.toLowerCase();
  if (/universal2?|universal-/.test(name)) return "universal";
  if (/aarch64|arm64/.test(name)) return "arm";
  if (/x86_64|x64|amd64/.test(name)) return "x64";
  return "any";
}

function findMacInstaller(releases, architecture) {
  for (const release of stableReleases(releases)) {
    const assets = release.assets.filter(isMacInstaller).sort((a, b) => {
      const rank = (asset) => {
        const name = asset.name.toLowerCase();
        return name.endsWith(".dmg") ? 0 : name.endsWith(".pkg") ? 1 : name.endsWith(".zip") ? 2 : 3;
      };
      return rank(a) - rank(b);
    });
    const universal = assets.find((asset) => macArchitecture(asset) === "universal");
    const matching = architecture && architecture !== "unknown"
      ? assets.find((asset) => macArchitecture(asset) === architecture)
      : null;
    const generic = assets.find((asset) => macArchitecture(asset) === "any");
    const onlyCompatibleAsset = assets.length === 1 &&
      (macArchitecture(assets[0]) === "universal" ||
        macArchitecture(assets[0]) === "any" ||
        macArchitecture(assets[0]) === architecture);
    if (universal || matching || generic || onlyCompatibleAsset) {
      return universal || matching || generic || assets[0];
    }
  }
  return null;
}

function installerUrl(installer) {
  if (typeof installer?.browser_download_url !== "string") return null;
  try {
    const url = new URL(installer.browser_download_url);
    return url.protocol === "https:" && url.hostname === "github.com" && url.pathname.startsWith(RELEASE_ASSET_PREFIX)
      ? url
      : null;
  } catch {
    return null;
  }
}

function sendUnavailable(response, platform) {
  response.statusCode = platform === "mac" ? 404 : 503;
  response.setHeader("Content-Type", "text/plain; charset=utf-8");
  response.setHeader("Cache-Control", "no-store");
  response.end(platform === "mac"
    ? "No compatible macOS installer is available."
    : "The latest Windows installer is currently unavailable.");
}

module.exports = async function downloadLatestInstaller(request, response) {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.setHeader("Allow", "GET, HEAD");
    response.statusCode = 405;
    response.end("Method not allowed");
    return;
  }

  const requestUrl = new URL(request.url || "/download", "https://capsule-meter.local");
  const platform = requestUrl.searchParams.get("platform") === "mac" ? "mac" : "windows";
  const architecture = requestUrl.searchParams.get("arch") || "unknown";

  try {
    const releaseResponse = await fetch(RELEASES_API, {
      headers: {
        accept: "application/vnd.github+json",
        "user-agent": "CapsuleMeter-Website",
        "x-github-api-version": "2022-11-28",
      },
      cache: "no-store",
    });

    if (!releaseResponse.ok) {
      sendUnavailable(response, platform);
      return;
    }

    const releases = await releaseResponse.json();
    const installer = platform === "mac"
      ? findMacInstaller(releases, architecture)
      : findWindowsInstaller(releases);
    const downloadUrl = installerUrl(installer);

    if (!installer || !downloadUrl) {
      sendUnavailable(response, platform);
      return;
    }

    if (request.method === "HEAD") {
      response.statusCode = 302;
      response.setHeader("Location", downloadUrl.href);
      response.setHeader("Cache-Control", "no-store");
      response.end();
      return;
    }

    const installerResponse = await fetch(downloadUrl.href, {
      headers: { accept: "application/octet-stream" },
    });

    if (!installerResponse.ok || !installerResponse.body) {
      sendUnavailable(response, platform);
      return;
    }

    const filename = installer.name.replace(/[\\/"\r\n]/g, "_");
    response.statusCode = 200;
    response.setHeader("Content-Type", "application/octet-stream");
    response.setHeader(
      "Content-Disposition",
      `attachment; filename="${filename}"; filename*=UTF-8''${encodeURIComponent(filename)}`
    );
    response.setHeader("Cache-Control", "no-store");
    response.setHeader("X-Content-Type-Options", "nosniff");
    if (Number.isFinite(installer.size) && installer.size > 0) {
      response.setHeader("Content-Length", String(installer.size));
    }

    Readable.fromWeb(installerResponse.body).pipe(response);
  } catch {
    if (response.headersSent) {
      response.destroy();
      return;
    }
    sendUnavailable(response, platform);
  }
};
