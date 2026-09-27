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

export default async function latestWindowsDownload(): Promise<Response> {
  try {
    const response = await fetch(RELEASE_API, {
      headers: {
        accept: "application/vnd.github+json",
        "user-agent": "CapsuleMeter-Website",
        "x-github-api-version": "2022-11-28",
      },
    });

    if (!response.ok) return redirect(RELEASE_PAGE, 15);

    const release = (await response.json()) as GitHubRelease;
    const installer = release.assets?.find((asset) =>
      asset.name?.toLowerCase().endsWith("_x64-setup.exe")
    );

    if (!installer?.browser_download_url) return redirect(RELEASE_PAGE, 15);

    const downloadUrl = new URL(installer.browser_download_url);
    if (
      downloadUrl.protocol !== "https:" ||
      downloadUrl.hostname !== "github.com" ||
      !downloadUrl.pathname.startsWith(RELEASE_ASSET_PREFIX)
    ) {
      return redirect(RELEASE_PAGE, 15);
    }

    return redirect(downloadUrl.href);
  } catch {
    return redirect(RELEASE_PAGE, 15);
  }
}

export const config: Config = {
  path: "/download",
  method: ["GET", "HEAD"],
  cache: "manual",
  onError: "bypass",
};
