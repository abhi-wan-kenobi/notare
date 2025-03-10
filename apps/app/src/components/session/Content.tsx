import type { Session } from "../../client";

interface ContentProps {
  session: Session;
}

export function Content({ session }: ContentProps) {
  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex-1 overflow-y-auto px-5 pb-20 pt-4 mx-auto max-w-2xl w-full">
        <div
          className="prose max-w-none"
          dangerouslySetInnerHTML={{
            __html: session.enhanced_memo_html || session.raw_memo_html,
          }}
        />
      </div>
    </div>
  );
}
