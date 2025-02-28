import { Button } from "@hypr/ui/components/ui/button";
import {
  WidgetFullSizeModal,
  WidgetHeader,
} from "@hypr/ui/components/ui/widgets";
import { Minimize2Icon } from "lucide-react";
import Notes from "../components/notes";

const PastNotesFull: typeof WidgetFullSizeModal = ({ onMinimize }) => {
  return (
    <WidgetFullSizeModal onMinimize={onMinimize}>
      <div className="p-4 pb-0">
        <WidgetHeader
          title={<div className="flex items-center gap-2">Past Notes</div>}
          actions={[
            <Button
              key="minimize"
              variant="ghost"
              size="icon"
              onClick={onMinimize}
            >
              <Minimize2Icon className="h-4 w-4" />
            </Button>,
          ]}
        />
      </div>

      <div className="overflow-auto flex-1 p-4 pt-0">
        <Notes notes={notes} />
      </div>
    </WidgetFullSizeModal>
  );
};

export default PastNotesFull;
