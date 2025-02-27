import { useEffect, useState } from "react";
import { getTimeForTimezone } from "../../lib";

export const ClockItem = ({ timezone, city }: { timezone: string; city: string }) => {
  const [time, setTime] = useState(getTimeForTimezone(timezone));

  useEffect(() => {
    const interval = setInterval(() => {
      setTime(getTimeForTimezone(timezone));
    }, 1000);

    return () => clearInterval(interval);
  }, [timezone]);

  return (
    <div className="flex flex-col items-center justify-center h-full w-full p-2">
      <div className="text-sm font-semibold mb-1">{city}</div>
      <div className="text-xl font-bold">{time.time}</div>
      <div className="text-xs text-gray-500">{time.date}</div>
    </div>
  );
};