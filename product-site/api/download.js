const { Readable } = require("node:stream");

const RELEASE_API = "https://api.github.com/repos/percyho/CapsuleMeter/releases/latest";
const RELEASES_API = "https://api.github.com/repos/percyho/CapsuleMeter/releases?per_page=100";
const RELEASE_ASSET_PREFIX = "/percyho/CapsuleMeter/releases/download/";

function unavailable(response) {
  response.statusCode = 503;
  response.setHeader("Content-Type", "text/plain; charset=utf-8");
  response.setHeader("Cache-Control", "no-store");
  response.end("The latest Windows installer is currently unavailable. Please try again later.");
}

function macInstallerUnavailable(response) {
  response.statusCode = 404;
  response.setHeader("Content-Type", "text/plain; charset=utf-8");
  response.setHeader("Cache-Control", "no-store");
  response.end("No compatible macOS installer is available.");
}

module.exports = async function downloadLatestWindowsInstaller(request, response) {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.setHeader("Allow", "GET, HEAD");
    response.statusCode = 405;
    response.end("Method not allowed");
    return;
  }

  try {
    const requestUrl = new URL(request.url || "/download", "https://capsule-meter.local");
    const platform = requestUrl.searchParams.get("platform") === "mac" ? "mac" : "windows";
    const architecture = requestUrl.searchParams.get("arch");
    const releaseResponse = await fetch(platform === "mac" ? RELEASES_API : RELEASE_API, {
      headers: {
        accept: "application/vnd.github+json",
        "user-agent": "CapsuleMeter-Website",
        "x-github-api-version": "2022-11-28",
      },
      cache: "no-store",
    });

    if (!releaseResponse.ok) {
      unavailable(response);
      return;
    }

    const releaseData = await releaseResponse.json();
    let installer;
    if (platform === "mac") {
      const macAsset = (asset) => {
        if (!asset || typeof asset.name !== "string" || typeof asset.browser_download_url !== "string") return false;
        const name = asset.name.toLowerCase();
        return /\.(dmg|pkg)$/i.test(name) || /\.app\.tar\.gz$/i.test(name) || (/\.zip$/i.test(name) && /mac|macos|osx|darwin|arm64|aarch64|x64|x86_64|universal/i.test(name));
      };
      const architectureOf = (asset) => {
        const name = asset.name.toLowerCase();
        if (/universal2?|universal-/.test(name)) return "universal";
        if (/aarch64|arm64/.test(name)) return "arm";
        if (/x86_64|x64|amd64/.test(name)) return "x64";
        return "any";
      };
      const releases = Array.isArray(releaseData) ? releaseData : [];
      const eligible = releases
        .filter((release) => release && !release.draft && !release.prerelease && Array.isArray(release.assets))
        .map((release) => ({
          release,
          assets: release.assets.filter(macAsset).sort((a, b) => {
            const rank = (asset) => asset.name.toLowerCase().endsWith(".dmg") ? 0 : asset.name.toLowerCase().endsWith(".pkg") ? 1 : asset.name.toLowerCase().endsWith(".zip") ? 2 : 3;
            return rank(a) - rank(b);
          }),
        }))
        .filter((item) => item.assets.length)
        .sort((a, b) => Date.parse(b.release.published_at || b.release.created_at || 0) - Date.parse(a.release.published_at || a.release.created_at || 0));

      for (const item of eligible) {
        const universal = item.assets.find((asset) => architectureOf(asset) === "universal");
        const matching = architecture && architecture !== "unknown"
          ? item.assets.find((asset) => architectureOf(asset) === architecture)
          : undefined;
        const generic = item.assets.find((asset) => architectureOf(asset) === "any");
        const onlyAsset = item.assets.length === 1 && (architectureOf(item.assets[0]) === "any" || architectureOf(item.assets[0]) === architecture || architectureOf(item.assets[0]) === "universal");
        if (universal || matching || generic || onlyAsset) {
          installer = universal || matching || generic || item.assets[0];
          break;
        }
      }
    } else {
      installer = releaseData.assets?.find((asset) =>
        typeof asset.name === "string" &&
        asset.name.toLowerCase().endsWith("_x64-setup.exe") &&
        typeof asset.browser_download_url === "string"
      );
    }

    if (!installer) {
      if (platform === "mac") macInstallerUnavailable(response);
      else unavailable(response);
      return;
    }

    const downloadUrl = new URL(installer.browser_download_url);
    if (
      downloadUrl.protocol !== "https:" ||
      downloadUrl.hostname !== "github.com" ||
      !downloadUrl.pathname.startsWith(RELEASE_ASSET_PREFIX)
    ) {
      unavailable(response);
      return;
    }

    if (request.method === "HEAD") {
      response.statusCode = 200;
      response.setHeader("Content-Type", "application/octet-stream");
      response.setHeader("Content-Disposition", `attachment; filename="${installer.name.replace(/[\\/"\r\n]/g, "_")}"`);
      response.setHeader("Cache-Control", "no-store");
      if (Number.isFinite(installer.size) && installer.size > 0) {
        response.setHeader("Content-Length", String(installer.size));
      }
      response.end();
      return;
    }

    const installerResponse = await fetch(downloadUrl.href, {
      headers: { accept: "application/octet-stream" },
    });

    if (!installerResponse.ok || !installerResponse.body) {
      unavailable(response);
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
    unavailable(response);
  }
};
