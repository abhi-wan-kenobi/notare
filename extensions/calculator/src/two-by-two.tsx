import { type Extension } from "@hypr/extension-utils";
import { WidgetTwoByTwo } from "@hypr/ui/components/ui/widgets";
import { useState, useEffect } from "react";
import { cn } from "@hypr/ui/lib/utils";

const widget: Extension["twoByTwo"] = () => (
  <WidgetTwoByTwo>
    <Calculator />
  </WidgetTwoByTwo>
);

const Calculator = () => {
  const [display, setDisplay] = useState("0");
  const [prevValue, setPrevValue] = useState<number | null>(null);
  const [operator, setOperator] = useState<string | null>(null);
  const [newNumber, setNewNumber] = useState(true);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [operation, setOperation] = useState<string>("");

  const handleNumber = (num: string) => {
    if (newNumber) {
      setDisplay(num);
      setOperation(prevValue !== null ? `${prevValue}${operator}${num}` : num);
      setNewNumber(false);
    } else {
      const newDisplay = display === "0" ? num : display + num;
      setDisplay(newDisplay);
      setOperation(prevValue !== null ? `${prevValue}${operator}${newDisplay}` : newDisplay);
    }
  };

  const handleOperator = (op: string) => {
    const current = parseFloat(display);
    if (prevValue === null) {
      setPrevValue(current);
      setOperation(`${current}${op}`);
    } else if (operator) {
      const result = calculate(prevValue, current, operator);
      setPrevValue(result);
      setDisplay(result.toString());
      setOperation(`${result}${op}`);
    }
    setOperator(op);
    setNewNumber(true);
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    // Prevent default behavior for calculator keys
    if (
      /^[0-9.]$/.test(e.key) ||
      ["+", "-", "*", "/", "Enter", "Escape", "Backspace", "="].includes(e.key)
    ) {
      e.preventDefault();
    }

    // Set active key based on input
    const keyMap: Record<string, string> = {
      "/": "÷",
      "*": "×",
      "-": "−",
      "+": "+",
      ".": ".",
      "Enter": "=",
      "=": "=",
      "Escape": "C",
      "Backspace": "⌫",
    };

    const mappedKey = keyMap[e.key] || (/^[0-9]$/.test(e.key) ? e.key : null);
    setActiveKey(mappedKey);

    // Numbers and decimal
    if (/^[0-9]$/.test(e.key)) {
      handleNumber(e.key);
    } else if (e.key === ".") {
      handleDecimal();
    }
    // Operators
    else if (e.key === "+") {
      handleOperator("+");
    } else if (e.key === "-") {
      handleOperator("-");
    } else if (e.key === "*") {
      handleOperator("×");
    } else if (e.key === "/") {
      handleOperator("÷");
    }
    // Enter or = for equals
    else if (e.key === "Enter" || e.key === "=") {
      handleEquals();
    }
    // Escape for clear
    else if (e.key === "Escape") {
      handleClear();
    }
    // Backspace for delete
    else if (e.key === "Backspace") {
      handleDelete();
    }
  };

  const handleKeyUp = () => {
    setActiveKey(null);
  };

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, []);

  const calculate = (a: number, b: number, op: string): number => {
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

  const handleEquals = () => {
    if (operator && prevValue !== null) {
      const current = parseFloat(display);
      const result = calculate(prevValue, current, operator);
      setDisplay(result.toString());
      setOperation(`${prevValue}${operator}${current}`);
      setPrevValue(null);
      setOperator(null);
      setNewNumber(true);
    }
  };

  const handleClear = () => {
    setDisplay("0");
    setPrevValue(null);
    setOperator(null);
    setNewNumber(true);
    setOperation("");
  };

  const handleDecimal = () => {
    if (newNumber) {
      setDisplay("0.");
      setOperation(prevValue !== null ? `${prevValue}${operator}0.` : "0.");
      setNewNumber(false);
    } else if (!display.includes(".")) {
      setDisplay(display + ".");
      setOperation(prevValue !== null ? `${prevValue}${operator}${display}.` : `${display}.`);
    }
  };

  const handleDelete = () => {
    if (display.length > 1) {
      const newDisplay = display.slice(0, -1);
      setDisplay(newDisplay);
      setOperation(prevValue !== null ? `${prevValue}${operator}${newDisplay}` : newDisplay);
    } else {
      setDisplay("0");
      setOperation(prevValue !== null ? `${prevValue}${operator}0` : "0");
    }
  };

  const Button = ({
    children,
    onClick,
    className,
    value,
  }: {
    children: React.ReactNode;
    onClick: () => void;
    className?: string;
    value: string;
  }) => (
    <button
      onClick={onClick}
      className={cn(
        "w-full h-full text-2xl font-light border border-neutral-200",
        activeKey === value 
          ? "bg-neutral-200" 
          : cn("hover:bg-neutral-100", className),
      )}
    >
      {children}
    </button>
  );

  return (
    <div className="h-full flex flex-col bg-white text-neutral-900 rounded-lg overflow-hidden">
      <div className="flex flex-col h-full">
        <div className="flex-none h-32 flex flex-col items-end justify-end p-4 overflow-hidden">
          <div className="text-2xl text-neutral-400 font-light">{operation || ""}</div>
          <div className="text-5xl font-light">{display}</div>
        </div>
        <div className="flex-1 grid grid-cols-4">
          <Button value="C" onClick={handleClear} className="text-neutral-600 bg-neutral-100">
            C
          </Button>
          <Button value="⌫" onClick={handleDelete} className="text-neutral-600 bg-neutral-100">
            ⌫
          </Button>
          <Button value="%" onClick={() => {}} className="text-neutral-600 bg-neutral-100">
            %
          </Button>
          <Button value="÷" onClick={() => handleOperator("÷")} className="bg-neutral-200">
            ÷
          </Button>

          <Button value="7" onClick={() => handleNumber("7")}>7</Button>
          <Button value="8" onClick={() => handleNumber("8")}>8</Button>
          <Button value="9" onClick={() => handleNumber("9")}>9</Button>
          <Button value="×" onClick={() => handleOperator("×")} className="bg-neutral-200">
            ×
          </Button>

          <Button value="4" onClick={() => handleNumber("4")}>4</Button>
          <Button value="5" onClick={() => handleNumber("5")}>5</Button>
          <Button value="6" onClick={() => handleNumber("6")}>6</Button>
          <Button value="−" onClick={() => handleOperator("-")} className="bg-neutral-200">
            −
          </Button>

          <Button value="1" onClick={() => handleNumber("1")}>1</Button>
          <Button value="2" onClick={() => handleNumber("2")}>2</Button>
          <Button value="3" onClick={() => handleNumber("3")}>3</Button>
          <Button value="+" onClick={() => handleOperator("+")} className="bg-neutral-200">
            +
          </Button>

          <Button value="0" onClick={() => handleNumber("0")} className="col-span-2">
            0
          </Button>
          <Button value="." onClick={handleDecimal}>.</Button>
          <Button value="=" onClick={handleEquals} className="bg-neutral-200">
            =
          </Button>
        </div>
      </div>
    </div>
  );
};

export default widget;
