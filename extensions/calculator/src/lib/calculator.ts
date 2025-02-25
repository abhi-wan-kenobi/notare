export type Operator = "+" | "-" | "×" | "÷";

export interface CalculatorState {
  display: string;
  prevValue: number | null;
  operator: Operator | null;
  operation: string;
  newNumber: boolean;
}

export const calculate = (a: number, b: number, op: Operator): number => {
  switch (op) {
    case "+":
      return a + b;
    case "-":
      return a - b;
    case "×":
      return a * b;
    case "÷":
      return b !== 0 ? a / b : 0;
    default:
      return b;
  }
};

export const handleNumber = (
  state: CalculatorState,
  num: string
): Partial<CalculatorState> => {
  if (state.newNumber) {
    return {
      display: num,
      operation: state.prevValue !== null ? `${state.prevValue}${state.operator}${num}` : num,
      newNumber: false,
    };
  } else {
    const newDisplay = state.display === "0" ? num : state.display + num;
    return {
      display: newDisplay,
      operation: state.prevValue !== null ? `${state.prevValue}${state.operator}${newDisplay}` : newDisplay,
    };
  }
};

export const handleOperator = (
  state: CalculatorState,
  op: Operator
): Partial<CalculatorState> => {
  const current = parseFloat(state.display);
  if (state.prevValue === null) {
    return {
      prevValue: current,
      operator: op,
      operation: `${current}${op}`,
      newNumber: true,
    };
  } else if (state.operator) {
    const result = calculate(state.prevValue, current, state.operator);
    return {
      display: result.toString(),
      prevValue: result,
      operator: op,
      operation: `${result}${op}`,
      newNumber: true,
    };
  }
  return {
    operator: op,
    newNumber: true,
  };
};

export const handleEquals = (state: CalculatorState): Partial<CalculatorState> => {
  if (state.operator && state.prevValue !== null) {
    const current = parseFloat(state.display);
    const result = calculate(state.prevValue, current, state.operator);
    return {
      display: result.toString(),
      operation: `${state.prevValue}${state.operator}${current}`,
      prevValue: null,
      operator: null,
      newNumber: true,
    };
  }
  return {};
};

export const handleClear = (): CalculatorState => ({
  display: "0",
  prevValue: null,
  operator: null,
  operation: "",
  newNumber: true,
});

export const handleDelete = (state: CalculatorState): Partial<CalculatorState> => {
  if (state.display.length > 1) {
    const newDisplay = state.display.slice(0, -1);
    return {
      display: newDisplay,
      operation: state.prevValue !== null ? `${state.prevValue}${state.operator}${newDisplay}` : newDisplay,
    };
  } else {
    return {
      display: "0",
      operation: state.prevValue !== null ? `${state.prevValue}${state.operator}0` : "0",
    };
  }
};

export const handleDecimal = (state: CalculatorState): Partial<CalculatorState> => {
  if (state.newNumber) {
    return {
      display: "0.",
      operation: state.prevValue !== null ? `${state.prevValue}${state.operator}0.` : "0.",
      newNumber: false,
    };
  } else if (!state.display.includes(".")) {
    return {
      display: state.display + ".",
      operation: state.prevValue !== null ? `${state.prevValue}${state.operator}${state.display}.` : `${state.display}.`,
    };
  }
  return {};
};
