import { useState, useEffect } from "react";
import type { Session } from "../../client";
import { RiAppleFill, RiWindowsFill, RiPhoneFill } from "@remixicon/react";

interface HeaderProps {
  session: Session;
}

export function Header({ session }: HeaderProps) {
  const [platform, setPlatform] = useState<string>("Mac");

  useEffect(() => {
    // Detect user's platform
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
    <>
      <div className="flex flex-row items-center justify-between sm:pl-8 px-4 pt-6">
        <div className="w-full flex items-center">
          <img 
            src="/hyprnote_icon.png" 
            alt="Hyprnote" 
            className="h-8 w-8 mr-3"
          />
          <h1 className="border-none bg-transparent text-2xl font-bold text-neutral-800 truncate">
            {session.title || "Untitled"}
          </h1>
        </div>
        <div className="flex items-center">
          <div className="flex items-center mr-1">
            <a
              href="https://hyprnote.com/download"
              className="inline-flex items-center justify-center gap-2 px-4 py-3 rounded-full text-base bg-blue-500 hover:bg-blue-600 text-white font-medium transition-colors whitespace-nowrap min-w-[240px]"
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
      </div>
      <SessionChips session={session} />
    </>
  );
}

interface SessionChipsProps {
  session: Session;
}

function SessionChips({ session }: SessionChipsProps) {
  return (
    <div className="-mx-1.5 flex flex-row items-center px-4 pb-4 pt-1 overflow-x-auto scrollbar-none whitespace-nowrap sm:px-8">
      {session.conversations.length > 0 && (
        <div className="flex items-center">
          <div className="rounded-full bg-neutral-100 px-3 py-1 text-sm text-neutral-700 hover:bg-neutral-200">
            {session.conversations.length} Conversation
            {session.conversations.length !== 1 ? "s" : ""}
          </div>
        </div>
      )}

      {session.conversations.length > 0 &&
        session.conversations.some((conv) => conv.diarizations.length > 0) && (
          <div className="flex items-center ml-2">
            <div className="border-l border-neutral-200 h-4 mx-2"></div>
            <div className="rounded-full bg-neutral-100 px-3 py-1 text-sm text-neutral-700 hover:bg-neutral-200">
              {
                session.conversations.flatMap((conv) => conv.diarizations)
                  .length
              }{" "}
              Participant
              {session.conversations.flatMap((conv) => conv.diarizations)
                .length !== 1
                ? "s"
                : ""}
            </div>
          </div>
        )}
    </div>
  );
}
