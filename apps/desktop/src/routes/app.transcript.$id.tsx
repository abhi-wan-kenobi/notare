import { createFileRoute } from "@tanstack/react-router";
import { ReplaceAllIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { commands as dbCommands } from "@hypr/plugin-db";
import TranscriptEditor from "@hypr/tiptap/transcript";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@hypr/ui/components/ui/popover";

export const Route = createFileRoute("/app/transcript/$id")({
  component: Component,
  loader: async ({ params: { id }, context: { onboardingSessionId } }) => {
    const participants = await dbCommands.sessionListParticipants(id);
    const timeline = onboardingSessionId
      ? await dbCommands.getTimelineViewOnboarding()
      : await dbCommands.getTimelineView(id);

    return { participants, timeline };
  },
});

function Component() {
  const { participants, timeline } = Route.useLoaderData();
  const editorRef = useRef(null);

  const content = {
    type: "doc",
    content: [
      {
        type: "speaker",
        attrs: { label: "" },
        content: (timeline?.items || []).flatMap((item) => item.text.split(" ")).filter(Boolean).map((word) => ({
          type: "word",
          content: [{ type: "text", text: word }],
        })),
      },
    ],
  };

  const [expanded, setExpanded] = useState(false);
  const [searchTerm, setSearchTerm] = useState("");
  const [replaceTerm, setReplaceTerm] = useState("");

  useEffect(() => {
    if (editorRef.current) {
      // @ts-ignore
      editorRef.current.editor.commands.setSearchTerm(searchTerm);

      if (searchTerm.substring(0, searchTerm.length - 1) === replaceTerm) {
        setReplaceTerm(searchTerm);
      }
    }
  }, [searchTerm]);

  useEffect(() => {
    if (editorRef.current) {
      // @ts-ignore
      editorRef.current.editor.commands.setReplaceTerm(replaceTerm);
    }
  }, [replaceTerm]);

  const handleReplaceAll = () => {
    if (editorRef.current && searchTerm) {
      // @ts-ignore
      editorRef.current.editor.commands.replaceAll(replaceTerm);
      // setExpanded(false);
      // TODO: we need editor state updated first.
    }
  };

  return (
    <div className="p-6 flex-1 flex flex-col overflow-hidden">
      <Popover open={expanded} onOpenChange={setExpanded}>
        <PopoverTrigger asChild>
          <Button
            className="w-8"
            variant="default"
            size="icon"
          >
            <ReplaceAllIcon size={12} />
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-full p-2">
          <div className="flex flex-row gap-2">
            <Input
              className="h-6"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Search"
            />
            <Input
              className="h-6"
              value={replaceTerm}
              onChange={(e) => setReplaceTerm(e.target.value)}
              placeholder="Replace"
            />
            <Button
              className="h-6"
              variant="default"
              onClick={handleReplaceAll}
            >
              Replace
            </Button>
          </div>
        </PopoverContent>
      </Popover>

      <div className="h-full overflow-auto">
        <TranscriptEditor
          ref={editorRef}
          initialContent={content}
          speakers={participants.map((p) => ({ id: p.id, name: p.full_name ?? "Unknown" }))}
        />
      </div>
    </div>
  );
}
