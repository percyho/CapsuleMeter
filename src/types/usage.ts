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

export interface ResetCredit {
  id: string;
  title: string | null;
  description: string | null;
  grantedAt: number;
  expiresAt: number | null;
  resetType: string;
  status: string;
}

export interface ResetCreditsSummary {
  availableCount: number;
  credits: ResetCredit[] | null;
}

export interface ConsumeResetResult {
  outcome: "reset" | "nothingToReset" | "noCredit" | "alreadyRedeemed";
}

export interface HistoryPoint {
  t: number;
  p?: number | null;
  fiveHour?: number | null;
  weekly?: number | null;
}
