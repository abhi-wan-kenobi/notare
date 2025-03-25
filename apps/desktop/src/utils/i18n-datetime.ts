import {
  formatRelative as originalFormatRelative,
  formatRemainingTime as originalFormatRemainingTime,
} from "@hypr/utils/datetime";
import { i18n } from "@lingui/core";

/**
 * Internationalized version of formatRemainingTime
 * Formats the remaining time until a date in a human-readable format with i18n support
 */
export function formatRemainingTime(date: Date): string {
  const now = new Date();
  const diff = date.getTime() - now.getTime();
  const days = Math.floor(diff / (1000 * 60 * 60 * 24));
  const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
  const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));

  if (days > 0) {
    return i18n._("{days} day{plural} later", {
      days,
      plural: days > 1 ? "s" : "",
    });
  } else if (hours > 0) {
    return i18n._("{hours} hour{plural} later", {
      hours,
      plural: hours > 1 ? "s" : "",
    });
  } else if (minutes > 1) {
    return i18n._("{minutes} minutes later", {
      minutes,
    });
  } else if (minutes > 0) {
    return i18n._("Starting soon");
  } else {
    return i18n._("In progress");
  }
}

/**
 * Internationalized version of formatRelative
 * Formats a date relative to the current date in a human-readable format with i18n support
 */
export function formatRelative(date: string, t?: string): string {
  const d = new Date(date);
  const now = new Date();

  // Calculate the difference in days
  const startOfDay = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const diffInDays = Math.floor((startOfToday.getTime() - startOfDay.getTime()) / (1000 * 60 * 60 * 24));

  if (diffInDays === 0) {
    return i18n._("Today");
  } else if (diffInDays === 1) {
    return i18n._("Yesterday");
  } else if (diffInDays === 2) {
    return i18n._("2 days ago");
  } else if (diffInDays < 7) {
    return i18n._("{days} days ago", { days: diffInDays });
  } else if (diffInDays < 14) {
    return i18n._("Last week");
  } else if (diffInDays < 21) {
    return i18n._("2 weeks ago");
  } else if (diffInDays < 30) {
    return i18n._("3 weeks ago");
  } else {
    return i18n._("{days} days ago", { days: diffInDays });
  }
}

/**
 * Formats a past date relative to now in a human-readable format with i18n support
 * Examples: "just now", "1 minute ago", "2 hours ago", "Yesterday", etc.
 */
export function formatTimeAgo(date: Date | string): string {
  const pastDate = typeof date === "string" ? new Date(date) : date;
  const now = new Date();
  const diff = now.getTime() - pastDate.getTime();

  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  const weeks = Math.floor(days / 7);
  const months = Math.floor(days / 30);
  const years = Math.floor(days / 365);

  if (seconds < 30) {
    return i18n._("just now");
  } else if (seconds < 60) {
    return i18n._("less than a minute ago");
  } else if (minutes === 1) {
    return i18n._("1 minute ago");
  } else if (minutes < 60) {
    return i18n._("{minutes} minutes ago", { minutes });
  } else if (hours === 1) {
    return i18n._("1 hour ago");
  } else if (hours < 24) {
    return i18n._("{hours} hours ago", { hours });
  } else if (days === 1) {
    return i18n._("Yesterday");
  } else if (days < 7) {
    return i18n._("{days} days ago", { days });
  } else if (weeks === 1) {
    return i18n._("1 week ago");
  } else if (weeks < 4) {
    return i18n._("{weeks} weeks ago", { weeks });
  } else if (months === 1) {
    return i18n._("1 month ago");
  } else if (months < 12) {
    return i18n._("{months} months ago", { months });
  } else if (years === 1) {
    return i18n._("1 year ago");
  } else {
    return i18n._("{years} years ago", { years });
  }
}

export { originalFormatRelative, originalFormatRemainingTime };
