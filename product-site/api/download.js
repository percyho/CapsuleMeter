const { Readable } = require("node:stream");

const RELEASE_API = "https://api.github.com/repos/percyho/CapsuleMeter/releases/latest";
const RELEASE_ASSET_PREFIX = "/percyho/CapsuleMeter/releases/download/";

function unavailable(response) {
  response.statusCode = 503;
  response.setHeader("Content-Type", "text/plain; charset=utf-8");
  response.setHeader("Cache-Control", "no-store");
  response.end("The latest Windows installer is currently unavailable. Please try again later.");
}

module.exports = async function downloadLatestWindowsInstaller(request, response) {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.setHeader("Allow", "GET, HEAD");
    response.statusCode = 405;
    response.end("Method not allowed");
    return;
  }

  try {
    const releaseResponse = await fetch(RELEASE_API, {
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

    const release = await releaseResponse.json();
    const installer = release.assets?.find((asset) =>
      typeof asset.name === "string" &&
      asset.name.toLowerCase().endsWith("_x64-setup.exe") &&
      typeof asset.browser_download_url === "string"
    );

    if (!installer) {
      unavailable(response);
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
