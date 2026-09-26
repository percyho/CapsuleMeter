import type { Config, Context } from "@netlify/edge-functions";

const CHINESE_LANGUAGE_COUNTRIES = new Set(["CN", "HK", "MO", "TW"]);

export default async function capsuleCountryLanguage(
  request: Request,
  context: Context,
) {
  const countryCode = context.geo.country?.code?.toUpperCase() ?? "";
  const language = CHINESE_LANGUAGE_COUNTRIES.has(countryCode) ? "zh" : "en";

  context.cookies.set({
    name: "capsule-ip-language",
    value: language,
    path: "/",
    maxAge: 60 * 60 * 24,
    sameSite: "lax",
    secure: new URL(request.url).protocol === "https:",
  });

  return context.next();
}

export const config: Config = {
  path: ["/CapsuleMeter*", "/product-site/CapsuleMeter*"],
  method: "GET",
  onError: "bypass",
};
