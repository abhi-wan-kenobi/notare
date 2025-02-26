import React, { useState, useEffect, useRef } from "react";
import { Play, Pause, RotateCcw } from "lucide-react";

interface TimerProps {
  initialTime?: number; // in minutes
}

export const Timer: React.FC<TimerProps> = ({ initialTime = 60 }) => {
  // Cap the initial time at 60 minutes
  const cappedInitialTime = Math.min(initialTime, 60);

  const [timeLeft, setTimeLeft] = useState(cappedInitialTime * 60); // Convert to seconds
  const [isRunning, setIsRunning] = useState(false);
  const [inputTime, setInputTime] = useState(cappedInitialTime.toString());
  const [isEditing, setIsEditing] = useState(false);
  const timerRef = useRef<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Check if timer is complete
  const isTimeUp = timeLeft === 0;

  useEffect(() => {
    if (isRunning && timeLeft > 0) {
      timerRef.current = window.setInterval(() => {
        setTimeLeft((prev) => prev - 1);
      }, 1000);
    } else if (!isRunning && timerRef.current) {
      clearInterval(timerRef.current);
    }

    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [isRunning, timeLeft]);

  useEffect(() => {
    if (timeLeft === 0) {
      setIsRunning(false);
    }
  }, [timeLeft]);

  const toggleTimer = () => {
    setIsRunning((prev) => !prev);
  };

  const resetTimer = () => {
    setIsRunning(false);
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    // Ensure inputTime is at least a minimum of 1 and a maximum of 60
    const parsedTime = Math.max(Math.min(parseInt(inputTime || "1"), 60), 1);
    setInputTime(parsedTime.toString());
    setTimeLeft(parsedTime * 60);
  };

  const handleTimeInput = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;

    // Only allow numbers
    if (/^\d*$/.test(value)) {
      // Update input value
      setInputTime(value);

      // Only update timer if not running
      if (!isRunning) {
        // Parse the value, defaulting to 1 if empty or 0, capping at 60
        const parsedValue = parseInt(value || "1");
        const validValue = Math.max(Math.min(parsedValue, 60), 1);

        // If the input was invalid (0 or empty) or beyond limits, correct the display
        if (parsedValue !== validValue) {
          setInputTime(validValue.toString());
        }

        setTimeLeft(validValue * 60);
      }
    }
  };

  const startEditing = () => {
    if (isRunning || isTimeUp) return;
    setIsEditing(true);
    setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 0);
  };

  const stopEditing = () => {
    // When stopping editing, ensure we have a valid value between 1 and 60
    const currentValue = parseInt(inputTime || "1");
    const validValue = Math.max(Math.min(currentValue, 60), 1);

    // Update if needed
    if (currentValue !== validValue) {
      setInputTime(validValue.toString());
      setTimeLeft(validValue * 60);
    }

    setIsEditing(false);
  };

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  };

  // Calculate progress with safety check
  const totalSeconds =
    Math.max(Math.min(parseInt(inputTime || "1"), 60), 1) * 60;
  const progress = (timeLeft / totalSeconds) * 360;

  // Generate minute markers
  const generateTickMarks = () => {
    const marks = [];

    // Add tick marks for each minute
    for (let i = 0; i < 60; i++) {
      const angle = (i / 60) * 360;
      const radian = (angle - 90) * (Math.PI / 180);
      const outerX = 50 + 45 * Math.cos(radian);
      const outerY = 50 + 45 * Math.sin(radian);
      const isMainTick = i % 5 === 0;
      const innerX = 50 + (isMainTick ? 40 : 42) * Math.cos(radian);
      const innerY = 50 + (isMainTick ? 40 : 42) * Math.sin(radian);

      marks.push(
        <line
          key={`tick-${i}`}
          x1={innerX}
          y1={innerY}
          x2={outerX}
          y2={outerY}
          stroke="black"
          strokeWidth={isMainTick ? "0.7" : "0.5"}
        />,
      );
    }

    return marks;
  };

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        position: "relative",
        backgroundColor: isTimeUp ? "rgba(255, 0, 0, 0.1)" : "transparent",
        transition: "background-color 0.5s ease",
      }}
    >
      {/* Button controls in corners - Only show Play/Pause when timer is not complete */}
      {!isTimeUp && (
        <button
          className="absolute top-2 left-2 z-20 p-1 cursor-pointer bg-white/70 rounded-full hover:bg-white/90 flex items-center justify-center"
          onClick={toggleTimer}
          style={{ width: 32, height: 32, border: "none" }}
        >
          {isRunning ? <Pause size={20} /> : <Play size={20} />}
        </button>
      )}

      {/* Always show Reset button - positioned in right corner */}
      <button
        className="absolute top-2 right-2 z-20 p-1 cursor-pointer bg-white/70 rounded-full hover:bg-white/90 flex items-center justify-center"
        onClick={resetTimer}
        style={{ width: 32, height: 32, border: "none" }}
      >
        <RotateCcw size={20} />
      </button>

      {/* Timer Display and Input */}
      <div className="absolute inset-0 flex items-center justify-center z-10">
        {isTimeUp ? (
          <div className="text-2xl font-bold text-red-600">Time's up!</div>
        ) : isEditing ? (
          <div className="flex flex-col items-center">
            <input
              ref={inputRef}
              type="number"
              min="1"
              max="60"
              value={inputTime}
              onChange={handleTimeInput}
              onBlur={stopEditing}
              onKeyDown={(e) => e.key === "Enter" && stopEditing()}
              className="w-16 text-center text-2xl font-bold bg-transparent border-none focus:outline-none"
              disabled={isRunning}
              style={{
                appearance: "textfield", // Remove spinners in Firefox
                MozAppearance: "textfield",
              }}
            />
          </div>
        ) : (
          <div
            className="flex justify-center items-center"
            onClick={startEditing}
          >
            <div className="text-2xl font-bold">{formatTime(timeLeft)}</div>
          </div>
        )}
      </div>

      {/* Explanatory text */}
      <div className="absolute bottom-2 left-2 text-xs text-gray-500 z-10">
        in minutes
      </div>

      {/* Explanatory text */}
      <div className="absolute bottom-2 right-2 text-xs text-gray-500 z-10">
        range: 1-60
      </div>

      {/* Timer Disc */}
      <svg
        viewBox="0 0 100 100"
        className="w-full h-full"
        preserveAspectRatio="xMidYMid meet"
      >
        {/* Background circle */}
        <circle
          cx="50"
          cy="50"
          r="45"
          fill={isTimeUp ? "#ffeeee" : "white"}
          stroke={isTimeUp ? "#ff6666" : "#e0e0e0"}
          strokeWidth={isTimeUp ? "3" : "2"}
        />

        {!isTimeUp && (
          /* Red progress circle - only show when timer is not complete */
          <path
            d={`
              M 50 50
              L 50 5
              A 45 45 0 ${progress > 180 ? 1 : 0} 1 ${
                50 + 45 * Math.sin((progress * Math.PI) / 180)
              } ${50 - 45 * Math.cos((progress * Math.PI) / 180)}
              Z
            `}
            fill="#ff0000"
          />
        )}

        {isTimeUp && (
          /* Full red circle for time's up state */
          <circle cx="50" cy="50" r="45" fill="#ff6666" opacity="0.7" />
        )}

        {/* Center white circle */}
        <circle
          cx="50"
          cy="50"
          r="12"
          fill="white"
          stroke={isTimeUp ? "#ff6666" : "#e0e0e0"}
          strokeWidth="1"
        />

        {/* Minute and tick marks - only show when timer is not complete */}
        {!isTimeUp && generateTickMarks()}
      </svg>
    </div>
  );
};
