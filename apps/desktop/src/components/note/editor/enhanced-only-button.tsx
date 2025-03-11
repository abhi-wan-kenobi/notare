import { Button } from "@hypr/ui/components/ui/button";
import { Zap } from "lucide-react";

interface EnhanceOnlyButtonProps {
  handleClick: () => void;
}

export function EnhanceOnlyButton({ handleClick }: EnhanceOnlyButtonProps) {
  return (
    <Button
      variant="default"
      size="lg"
      onClick={handleClick}
      className="hover:scale-95 transition-all"
    >
      <Zap size={20} />
      Hypercharge
    </Button>
  );
}
