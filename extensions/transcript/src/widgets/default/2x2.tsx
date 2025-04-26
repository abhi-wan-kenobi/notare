import { Maximize2Icon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { WidgetTwoByTwo, WidgetTwoByTwoWrapper } from "@hypr/ui/components/ui/widgets";

import { TranscriptBase } from "./base";

const Transcript2x2: WidgetTwoByTwo = ({ onMaximize, queryClient }) => {
  const maximizeButton = (
    <Button
      key="maximize"
      variant="ghost"
      size="icon"
      onClick={onMaximize}
      className="p-0"
    >
      <Maximize2Icon size={16} className="text-neutral-900" />
    </Button>
  );

  return (
    <TranscriptBase
      sizeToggleButton={maximizeButton}
      WrapperComponent={WidgetTwoByTwoWrapper}
      wrapperProps={{ queryClient }}
    />
  );
};

export default Transcript2x2;
