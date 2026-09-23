export type CapsuleTheme =
  | "realistic"
  | "flat"
  | "skeuomorphic"
  | "pixel"
  | "jelly"
  | "neon";

export interface CapsuleThemeOption {
  id: CapsuleTheme;
  name: string;
  description: string;
}

export const capsuleThemeOptions: readonly CapsuleThemeOption[] = [
  { id: "realistic", name: "玻璃拟态", description: "磨砂半透明与柔和光晕" },
  { id: "flat", name: "极简扁平", description: "扁平色块与干净克制" },
  { id: "skeuomorphic", name: "立体拟物", description: "立体光影与自然阴影" },
  { id: "jelly", name: "液态果冻", description: "高光饱满与柔软弹性" },
  { id: "pixel", name: "像素游戏", description: "像素轮廓与复古游戏感" },
  { id: "neon", name: "霓虹夜光", description: "双色霓虹描边与深夜光晕" },
] as const;

export function parseCapsuleTheme(value: string | null): CapsuleTheme {
  if (value === "beads") return "pixel";
  if (value === "industrial") return "realistic";
  if (value === "monochrome") return "flat";
  if (value === "y2k") return "jelly";
  if (value === "metal" || value === "terminal") return "realistic";
  return capsuleThemeOptions.some((option) => option.id === value)
    ? value as CapsuleTheme
    : "realistic";
}
