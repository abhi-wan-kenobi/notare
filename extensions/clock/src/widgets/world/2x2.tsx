import { WidgetTwoByTwo } from "@hypr/ui/components/ui/widgets";
import { ClockItem } from "../components/clock-item";

const WorldClock2x2: typeof WidgetTwoByTwo = () => {
  const timezones = [
    { timezone: "America/New_York", city: "New York" },
    { timezone: "Europe/London", city: "London" },
    { timezone: "Asia/Tokyo", city: "Tokyo" },
    { timezone: "Australia/Sydney", city: "Sydney" },
  ];

  return (
    <WidgetTwoByTwo>
      <div className="grid grid-cols-2 grid-rows-2 h-full w-full">
        {timezones.map((tz, index) => (
          <ClockItem key={index} timezone={tz.timezone} city={tz.city} />
        ))}
      </div>
    </WidgetTwoByTwo>
  );
};

export default WorldClock2x2;
