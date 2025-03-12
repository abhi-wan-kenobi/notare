import { motion } from "motion/react";
import { getRandomValues } from "./utils";

export type EqualizerStickProps = {
  baseLength: number;
  amplitude: number;
};

export default function EqualizerStick({ baseLength, amplitude }: EqualizerStickProps) {
  // Scale the base length based on amplitude
  const scaledBaseLength = baseLength * Math.max(0.2, amplitude);

  return (
    <motion.div
      className="rounded-full bg-neutral-300"
      style={{ width: "2px" }}
      animate={{
        height: getRandomValues(16, 6, scaledBaseLength),
      }}
      transition={{
        duration: 1.1,
        ease: "easeInOut",
        times: [0.2, 0.3, 0.5, 0.7, 1.1, 1.3, 1.7],
        repeat: Infinity,
      }}
    />
  );
}
