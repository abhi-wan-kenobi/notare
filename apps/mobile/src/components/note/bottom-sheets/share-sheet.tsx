import { GlobeIcon } from "lucide-react";

import { BottomSheet, BottomSheetContent } from "@hypr/ui/components/ui/bottom-sheet";
import { Button } from "@hypr/ui/components/ui/button";

interface ShareSheetProps {
  open: boolean;
  onClose: () => void;
  onPublish?: () => void;
}

export function ShareSheet({ open, onClose, onPublish }: ShareSheetProps) {
  return (
    <BottomSheet open={open} onClose={onClose}>
      <BottomSheetContent className="bg-white">
        <div className="flex flex-col gap-4 p-4">
          <div className="text-center">
            <h3 className="text-lg font-medium mb-1">Publish your note</h3>
            <p className="text-sm text-neutral-600">
              Anyone with the link can view this page
            </p>
          </div>

          <Button size="lg" onClick={onPublish}>
            <GlobeIcon className="size-4 mr-2" /> Make it public
          </Button>
        </div>
      </BottomSheetContent>
    </BottomSheet>
  );
}
