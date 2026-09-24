import { readonly, shallowRef } from "vue";

export type AppLocale = "zh-CN" | "en-US";

const stored = localStorage.getItem("appLocale");
const locale = shallowRef<AppLocale>(stored === "en-US" ? "en-US" : "zh-CN");

const en: Record<string, string> = {
  "设置": "Settings",
  "个性化用量胶囊": "Customize Codex Capsule",
  "数据异常": "Data unavailable",
  "未登录": "Signed out",
  "关闭设置": "Close settings",
  "设置分类": "Settings sections",
  "外观": "Appearance",
  "行为": "Behavior",
  "统计": "Usage",
  "重置": "Reset",
  "系统": "System",
  "界面主题": "Theme",
  "明亮": "Light",
  "暗黑": "Dark",
  "透明度": "Opacity",
  "字号": "Text size",
  "窗口置顶": "Always on top",
  "贴边吸附": "Snap to edges",
  "显示模式": "Display mode",
  "显示用量数值": "Show usage values",
  "剩余": "Remaining",
  "已用": "Used",
  "重置显示时长": "Reset-time display",
  "数据": "Data",
  "自动刷新": "Auto refresh",
  "上次刷新": "Last refreshed",
  "刷新中…": "Refreshing…",
  "立即刷新": "Refresh now",
  "胶囊主题": "Capsule theme",
  "像素": "Pixel",
  "选择胶囊的材质与读数风格": "Choose the capsule material and readout style",
  "极简扁平": "Minimal flat",
  "扁平色块与干净克制": "Flat color blocks, clean and restrained",
  "立体拟物": "Skeuomorphic",
  "立体光影与自然阴影": "Dimensional lighting and natural shadows",
  "像素游戏": "Pixel game",
  "像素轮廓与复古游戏感": "Pixel outlines with retro game character",
  "霓虹夜光": "Neon night",
  "双色霓虹描边与深夜光晕": "Dual neon outlines with a deep-night glow",
  "历史保留": "History retention",
  "限额重置": "Usage reset",
  "开机自启": "Launch at startup",
  "显示胶囊": "Show capsule",
  "托盘图标": "Tray icon",
  "用量环": "Usage rings",
  "低额度通知": "Low-usage alerts",
  "提醒阈值": "Alert threshold",
  "全局快捷键": "Global shortcut",
  "诊断与更新": "Diagnostics & updates",
  "重新登录": "Sign in again",
  "连接诊断": "Run diagnostics",
  "检查更新": "Check for updates",
  "关于": "About",
  "版本": "Version",
  "隐私": "Privacy",
  "仅本地处理": "LOCAL ONLY",
  "许可证": "License",
  "恢复默认": "Restore defaults",
  "退出应用": "Quit app",
  "语言": "Language",
  "跟随系统": "System",
  "使用限额重置": "Usage limit reset",
  "完全重置（每周 + 5 小时）": "Full reset (weekly + 5-hour)",
  "无过期时间": "No expiration",
  "当前没有可用的限额重置机会": "No usage resets are currently available",
  "处理中…": "Applying…",
  "确认使用": "Confirm reset",
  "使用重置": "Use reset",
  "刷新": "Refresh",
  "重置机会使用后不可撤销，再次点击确认。": "A reset cannot be undone. Click again to confirm.",
  "额度历史": "Usage history",
  "Token 活动": "Token activity",
  "详细历史 ↗": "Full history ↗",
  "5 小时": "5-hour",
  "每周": "Weekly",
  "当前": "Current",
  "当前剩余": "Current",
  "最低剩余": "Lowest",
  "采样点数": "Samples",
  "记录跨度": "Time span",
  "全部": "All",
  "加载中…": "Loading…",
  "Token 活动不可用": "Token activity unavailable",
  "本范围总量": "Total",
  "本范围峰值": "Peak",
  "日均": "Daily avg.",
  "活跃天数": "Active days",
  "还没有记录额度历史": "No usage history yet",
  "还没有记录 Token 活动": "No token activity yet",
  "数据保存在本机，统计图不会上传历史记录。": "History and charts stay on this device.",
  "Codex Capsule 不提供账号系统，也不会收集、出售或同步你的使用数据。": "Codex Capsule has no account system and does not collect, sell, or sync your usage data.",
  "认证信息": "Credentials",
  "仅在本机读取 Codex 登录凭据；访问令牌不会显示在界面或写入历史记录。": "Codex credentials are read locally; access tokens are never displayed or written to history.",
  "网络请求": "Network",
  "仅用于向 ChatGPT 官方接口获取额度与用量统计，不会发送给第三方服务。": "Requests only retrieve usage from official ChatGPT services and are not sent to third parties.",
  "本地数据": "Local data",
  "偏好设置和额度采样保存在本机，可通过“恢复默认”或“清除历史”删除。": "Preferences and usage samples stay locally and can be removed by restoring defaults or clearing history.",
  "CSV 导出": "CSV export",
  "只有你主动选择保存位置时才会生成文件，应用不会自动上传导出内容。": "Files are created only when you choose an export location and are never uploaded automatically.",
  "遥测": "Telemetry",
  "应用不包含广告、用户追踪或后台遥测。": "The app includes no ads, tracking, or background telemetry.",
};

function t(key: string): string {
  return locale.value === "en-US" ? (en[key] ?? key) : key;
}

function setLocale(value: AppLocale) {
  locale.value = value;
  localStorage.setItem("appLocale", value);
  document.documentElement.lang = value;
}

document.documentElement.lang = locale.value;

export function useLocale() {
  return { locale: readonly(locale), setLocale, t };
}
