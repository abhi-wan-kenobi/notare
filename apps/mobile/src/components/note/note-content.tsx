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
    <div
      className="flex-1 flex flex-col overflow-hidden"
      onClick={() => {
        editorRef.current?.editor?.commands?.focus();
      }}
    >
      <div className="overflow-y-auto w-full py-6 flex-1">
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
