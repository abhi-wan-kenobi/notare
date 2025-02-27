export const getTimeForTimezone = (
  timezone: string,
): { time: string; date: string } => {
  const now = new Date();
  const options: Intl.DateTimeFormatOptions = {
    timeZone: timezone,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: true,
  };

  const dateOptions: Intl.DateTimeFormatOptions = {
    timeZone: timezone,
    weekday: "short",
    month: "short",
    day: "numeric",
  };

  const timeString = new Intl.DateTimeFormat("en-US", options).format(now);
  const dateString = new Intl.DateTimeFormat("en-US", dateOptions).format(now);

  return { time: timeString, date: dateString };
};
