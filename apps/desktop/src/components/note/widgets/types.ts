export type WidgetSize = "small" | "medium" | "large";

export interface Widget {
  id: string;
  type: string;
  size: WidgetSize;
  order?: number;
}
