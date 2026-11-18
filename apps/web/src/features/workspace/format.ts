import type { CaseStatus } from '../../types';
import type { ReviewCategory, TimePrecision } from './types';

/** Case-local display zone for the synthetic cases. Source time zones are shown separately. */
export const CASE_TIME_ZONE = 'Asia/Kolkata';
export const CASE_TIME_ZONE_LABEL = 'IST';

const dateTime = new Intl.DateTimeFormat('en-GB', {
  timeZone: CASE_TIME_ZONE,
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
});

const time = new Intl.DateTimeFormat('en-GB', {
  timeZone: CASE_TIME_ZONE,
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  hour12: false,
});

const dateOnly = new Intl.DateTimeFormat('en-GB', { timeZone: CASE_TIME_ZONE, day: '2-digit', month: 'short', year: 'numeric' });

export function formatDateTime(iso: string): string {
  return `${dateTime.format(new Date(iso))} ${CASE_TIME_ZONE_LABEL}`;
}

export function formatDate(iso: string): string {
  return dateOnly.format(new Date(iso));
}

/** Format a UTC timestamp at the precision the source actually supports. */
export function formatEventTime(iso: string, precision: TimePrecision): string {
  const full = time.format(new Date(iso));
  switch (precision) {
    case 'EXACT_SECOND':
      return full;
    case 'MINUTE':
      return full.slice(0, 5);
    case 'HOUR':
      return `${full.slice(0, 2)}:—`;
    case 'DAY':
      return formatDate(iso);
    case 'ESTIMATED':
    default:
      return `≈ ${full.slice(0, 5)}`;
  }
}

export function formatUtc(iso: string): string {
  return `${iso.replace('T', ' ').replace(/\.\d+Z$|Z$/, '')} UTC`;
}

export function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

export function shortHash(hash: string): string {
  return `${hash.slice(0, 4)}…${hash.slice(-4)}`;
}

export const CASE_STATUS_LABEL: Record<CaseStatus, string> = {
  DRAFT: 'Draft',
  ACTIVE: 'Active',
  UNDER_REVIEW: 'Under review',
  ARCHIVED: 'Archived',
};

export const PRECISION_LABEL: Record<TimePrecision, string> = {
  EXACT_SECOND: 'Exact (second)',
  MINUTE: 'Minute',
  HOUR: 'Hour',
  DAY: 'Day',
  ESTIMATED: 'Estimated',
};

export const REVIEW_CATEGORY_LABEL: Record<ReviewCategory, string> = {
  conflicting: 'Conflicting',
  uncertain: 'Uncertain relationship',
  missing: 'Missing information',
  unverified: 'Unverified',
  supporting: 'Supporting',
};

export function durationLabel(fromIso: string, toIso: string): string {
  const seconds = Math.round((new Date(toIso).getTime() - new Date(fromIso).getTime()) / 1000);
  const sign = seconds < 0 ? '−' : '+';
  const abs = Math.abs(seconds);
  const m = Math.floor(abs / 60);
  const s = abs % 60;
  return m > 0 ? `${sign}${m} min ${s} s` : `${sign}${s} s`;
}
