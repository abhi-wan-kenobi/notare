import { useRef } from "react";

import { type Session } from "@hypr/plugin-db";
import Editor, { TiptapEditor } from "@hypr/tiptap/editor";
import { useNote } from "../hooks/use-note";

interface ContentProps {
  session: Session;
}

export function NoteContent({ session }: ContentProps) {
  const editorRef = useRef<{ editor: TiptapEditor }>(null);
  const { handleEditorChange, content } = useNote({ session });

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex-1 overflow-y-auto w-full">
        <Editor
          ref={editorRef}
          handleChange={handleEditorChange}
          initialContent={content}
          autoFocus={false}
        />
      </div>
    </div>
  );
}
