import { format } from "date-fns";
import { create } from "zustand";

type CalendarSelectionState = {
  selectedDate: string | null;
  selectDate: (date: Date) => void;
  clearSelection: () => void;
};

export const useCalendarSelection = create<CalendarSelectionState>((set) => ({
  selectedDate: null,
  selectDate: (date) => set({ selectedDate: format(date, "yyyy-MM-dd") }),
  clearSelection: () => set({ selectedDate: null }),
}));
