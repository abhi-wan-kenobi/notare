import { AudioLinesIcon, MicIcon } from "lucide-react";

import { BottomSheet, BottomSheetContent } from "@hypr/ui/components/ui/bottom-sheet";
import { Button } from "@hypr/ui/components/ui/button";

interface RecordingSheetProps {
  open: boolean;
  onClose: () => void;
  onUploadFile: () => void;
  onStartRecord: () => void;
}

export function NewNoteSelectionSheet({
  open,
  onClose,
  onUploadFile,
  onStartRecord,
}: RecordingSheetProps) {
  return (
    <BottomSheet open={open} onClose={onClose}>
      <BottomSheetContent className="flex gap-2 bg-white">
        <Button
          className="aspect-square w-full flex-col gap-2 text-red-500 hover:bg-red-100 hover:text-red-600"
          variant="outline"
          onClick={onUploadFile}
        >
          <AudioLinesIcon size={32} />
          Upload recording
        </Button>
        <Button
          className="aspect-square w-full flex-col gap-2 bg-red-500 hover:bg-red-600 hover:text-red-50"
          onClick={onStartRecord}
        >
          <MicIcon size={32} />
          Start recording
        </Button>
      </BottomSheetContent>
    </BottomSheet>
  );
}
