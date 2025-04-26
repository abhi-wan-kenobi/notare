import { Minimize2Icon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { WidgetFullSize, WidgetFullSizeWrapper } from "@hypr/ui/components/ui/widgets";

import { TranscriptBase } from "./base";

const TranscriptFull: WidgetFullSize = ({ onMinimize }) => {
  const minimizeButton = (
    <Button key="minimize" variant="ghost" size="icon" onClick={onMinimize}>
      <Minimize2Icon className="h-4 w-4 text-black" />
    </Button>
  );

  return (
    <TranscriptBase
      sizeToggleButton={minimizeButton}
      WrapperComponent={WidgetFullSizeWrapper}
      wrapperProps={{ onMinimize }}
    />
  );
};

export default TranscriptFull;
