import { useState, useEffect } from "react";
import type { Session } from "../../client";
import { RiAppleFill, RiWindowsFill, RiPhoneFill } from "@remixicon/react";
import { SessionInfo } from "./session-info";

interface HeaderProps {
  session: Session;
}

export function Header({ session }: HeaderProps) {
  const [platform, setPlatform] = useState<string>("Mac");

  useEffect(() => {
    const userAgent = navigator.userAgent.toLowerCase();
    if (userAgent.includes("windows")) {
      setPlatform("Windows");
    } else if (
      userAgent.includes("iphone") ||
      userAgent.includes("ipad") ||
      userAgent.includes("ipod")
    ) {
      setPlatform("iOS");
    } else {
      setPlatform("Mac");
    }
  }, []);

  return (
    <div className="flex flex-col">
      <div className="flex flex-row items-center justify-between p-4 border-b">
        <div className="w-full flex items-center">
          <img
            src="/hyprnote_icon.png"
            alt="Hyprnote"
            className="h-8 w-8 mr-3"
          />
          <h1 className="border-none font-racing-sans bg-transparent text-2xl font-bold text-neutral-800 truncate">
            HYPRNOTE
          </h1>
        </div>

        <div className="flex items-center">
          <a
            href="https://hyprnote.com/download"
            className="inline-flex items-center justify-center gap-2 px-4 py-3 transition-all rounded-full text-sm bg-black hover:bg-neutral-800 hover:scale-95 text-white font-medium whitespace-nowrap min-w-[240px]"
          >
            {platform === "Mac" && (
              <>
                <RiAppleFill className="w-5 h-5 flex-shrink-0" />
                <div>Download Hyprnote for Mac</div>
              </>
            )}
            {platform === "Windows" && (
              <>
                <RiWindowsFill className="w-5 h-5 flex-shrink-0" />
                <div>Download Hyprnote for Windows</div>
              </>
            )}
            {platform === "iOS" && (
              <>
                <RiPhoneFill className="w-5 h-5 flex-shrink-0" />
                <div>Download Hyprnote for iOS</div>
              </>
            )}
          </a>
        </div>
      </div>

      <SessionInfo session={session} />
    </div>
  );
}
