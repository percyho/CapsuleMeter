export type CapsuleTheme =
  | "flat"
  | "skeuomorphic"
  | "pixel"
  | "neon";

export interface CapsuleThemeOption {
  id: CapsuleTheme;
  name: string;
  description: string;
}

export const capsuleThemeOptions: readonly CapsuleThemeOption[] = [
  { id: "flat", name: "极简扁平", description: "扁平色块与干净克制" },
  { id: "skeuomorphic", name: "立体拟物", description: "立体光影与自然阴影" },
  { id: "pixel", name: "像素游戏", description: "像素轮廓与复古游戏感" },
  { id: "neon", name: "霓虹夜光", description: "双色霓虹描边与深夜光晕" },
] as const;

export function parseCapsuleTheme(value: string | null): CapsuleTheme {
  if (value === "beads") return "pixel";
  if (value === "monochrome") return "flat";
  if (["realistic", "industrial", "jelly", "y2k", "metal", "terminal"].includes(value ?? "")) {
    return "flat";
  }
  return capsuleThemeOptions.some((option) => option.id === value)
    ? value as CapsuleTheme
    : "flat";
}
