import { Badge } from "@hypr/ui/components/ui/badge";
import { BottomSheet, BottomSheetContent } from "@hypr/ui/components/ui/bottom-sheet";

interface Tag {
  id: string;
  name: string;
}

interface TagsSheetProps {
  open: boolean;
  onClose: () => void;
  tags: Tag[];
}

export function TagsSheet({ open, onClose, tags }: TagsSheetProps) {
  return (
    <BottomSheet open={open} onClose={onClose}>
      <BottomSheetContent className="bg-white">
        <div className="p-4 space-y-4">
          <h3 className="text-lg font-medium mb-2">Tags</h3>

          <div className="flex flex-wrap gap-2">
            {tags.map((tag) => (
              <Badge
                key={tag.id}
                className="px-2 py-1"
                variant="outline"
              >
                {tag.name}
              </Badge>
            ))}
          </div>
        </div>
      </BottomSheetContent>
    </BottomSheet>
  );
}
