import type { Config } from "@netlify/edge-functions";

const RELEASE_API = "https://api.github.com/repos/percyho/CapsuleMeter/releases/latest";
const RELEASE_PAGE = "https://github.com/percyho/CapsuleMeter/releases/latest";
const RELEASE_ASSET_PREFIX = "/percyho/CapsuleMeter/releases/download/";

type GitHubRelease = {
  assets?: Array<{
    name?: string;
    browser_download_url?: string;
  }>;
};

function redirect(location: string, cacheSeconds = 60): Response {
  return new Response(null, {
    status: 302,
    headers: {
      location,
      "Netlify-CDN-Cache-Control": `public, s-maxage=${cacheSeconds}, must-revalidate`,
      "Cache-Control": "public, max-age=0, must-revalidate",
    },
  });
}

function progressError(message: string): Response {
  return new Response(message, {
    status: 502,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Expose-Headers": "Content-Length, Content-Disposition, Content-Type",
      "Cache-Control": "no-store",
      "Content-Type": "text/plain; charset=utf-8",
    },
  });
}

export default async function latestWindowsDownload(request: Request): Promise<Response> {
  const streamForProgress = new URL(request.url).searchParams.get("progress") === "1";

  try {
    const response = await fetch(RELEASE_API, {
      headers: {
        accept: "application/vnd.github+json",
        "user-agent": "CapsuleMeter-Website",
        "x-github-api-version": "2022-11-28",
      },
    });

    if (!response.ok) {
      return streamForProgress
        ? progressError("Could not load the latest release")
        : redirect(RELEASE_PAGE, 15);
    }

    const release = (await response.json()) as GitHubRelease;
    const installer = release.assets?.find((asset) =>
      asset.name?.toLowerCase().endsWith("_x64-setup.exe")
    );

    if (!installer?.name || !installer.browser_download_url) {
      return streamForProgress
        ? progressError("No Windows installer was found")
        : redirect(RELEASE_PAGE, 15);
    }

    const downloadUrl = new URL(installer.browser_download_url);
    if (
      downloadUrl.protocol !== "https:" ||
      downloadUrl.hostname !== "github.com" ||
      !downloadUrl.pathname.startsWith(RELEASE_ASSET_PREFIX)
    ) {
      return streamForProgress
        ? progressError("The installer URL was not accepted")
        : redirect(RELEASE_PAGE, 15);
    }

    if (streamForProgress) {
      const assetResponse = await fetch(downloadUrl.href);
      if (!assetResponse.ok || !assetResponse.body) {
        return progressError("Could not stream the Windows installer");
      }

      const safeFilename = installer.name.replace(/[\\/:*?"<>|\r\n]/g, "_");
      const headers = new Headers({
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Expose-Headers": "Content-Length, Content-Disposition, Content-Type",
        "Cache-Control": "no-store",
        "Content-Type": assetResponse.headers.get("content-type") || "application/octet-stream",
        "Content-Disposition": assetResponse.headers.get("content-disposition") ||
          `attachment; filename="${safeFilename}"`,
        "X-Content-Type-Options": "nosniff",
      });
      const contentLength = assetResponse.headers.get("content-length");
      if (contentLength) headers.set("Content-Length", contentLength);

      return new Response(assetResponse.body, { status: 200, headers });
    }

    return redirect(downloadUrl.href);
  } catch {
    return streamForProgress
      ? progressError("The Windows installer is temporarily unavailable")
      : redirect(RELEASE_PAGE, 15);
  }
}

export const config: Config = {
  path: "/download",
  method: "GET",
  cache: "manual",
  onError: "bypass",
};
