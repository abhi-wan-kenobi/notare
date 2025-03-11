import { useSearchPalette } from "@/contexts/search-palette";
import { cn } from "@/utils";
import { Button } from "@hypr/ui/components/ui/button";
import { SearchIcon } from "lucide-react";

export function SearchIconButton({ isShown }: { isShown: boolean }) {
  const { open } = useSearchPalette();

  return (
    <Button
      variant="ghost"
      size="icon"
      className={cn("hover:bg-neutral-200", isShown ? "block" : "md:hidden")}
      onClick={open}
      aria-label="Search"
    >
      <SearchIcon className="size-4" />
    </Button>
  );
}
