import { useRef } from "react";

import { type Session } from "@hypr/plugin-db";
import Editor, { TiptapEditor } from "@hypr/tiptap/editor";

interface ContentProps {
  session: Session;
}

export function NoteContent({ session }: ContentProps) {
  const editorRef = useRef<{ editor: TiptapEditor }>(null);

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex-1 overflow-y-auto w-full">
        <Editor
          ref={editorRef}
          handleChange={() => {
            // TODO: implement
          }}
          initialContent={session.enhanced_memo_html || session.raw_memo_html}
          autoFocus={false}
        />
      </div>
    </div>
  );
}
