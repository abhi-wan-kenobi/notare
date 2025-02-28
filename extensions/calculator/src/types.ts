export type Operator = "+" | "-" | "×" | "÷";

export interface CalculatorState {
  display: string;
  prevValue: number | null;
  operator: Operator | null;
  operation: string;
  newNumber: boolean;
}
