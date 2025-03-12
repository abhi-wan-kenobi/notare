/**
 * Generates an array of random values for animation
 * @param max Maximum value range
 * @param length Number of values to generate
 * @param baseLength Base length to scale values (as percentage)
 * @returns Array of random values that loops back to the first value
 */
export const getRandomValues = (max: number, length: number, baseLength: number) => {
  const values: number[] = [];
  for (let i = 0; i < length - 1; i++) {
    values.push((Math.random() * max - max / 2) + (baseLength / 100) * max);
  }
  values.push(values[0]); // Loop back to the first value for smooth looping animation
  return values;
};
