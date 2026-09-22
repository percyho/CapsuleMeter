export interface WindowData {
  remaining_percent: number | null;
  used_percent: number | null;
  reset_at: number | null;
  reset_after_seconds: number | null;
}

export interface UsageData {
  plan: string | null;
  five_hour: WindowData;
  weekly: WindowData;
  error: string | null;
}

export interface HistoryPoint {
  t: number;
  p?: number | null;
  fiveHour?: number | null;
  weekly?: number | null;
}
