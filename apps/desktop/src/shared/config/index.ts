import {
  useStoredSettingValues,
  type StoredSettingValues,
} from "~/settings/queries";
import {
  SETTING_DEFINITIONS,
  type SettingKey,
  type SettingValue,
} from "~/settings/schema";

type JsonParsedKeys =
  | "spoken_languages"
  | "personalization_dictionary_terms"
  | "ignored_platforms"
  | "included_platforms";

type ConfigValueType<K extends SettingKey> = K extends JsonParsedKeys
  ? string[]
  : K extends keyof typeof SETTING_DEFINITIONS
    ? "default" extends keyof (typeof SETTING_DEFINITIONS)[K]
      ? SettingValue<K>
      : SettingValue<K> | undefined
    : never;

const JSON_PARSED_KEYS = new Set<SettingKey>([
  "spoken_languages",
  "personalization_dictionary_terms",
  "ignored_platforms",
  "included_platforms",
]);

export function useConfigValue<K extends SettingKey>(
  key: K,
): ConfigValueType<K> {
  return resolveConfigValue(key, useStoredSettingValues());
}

export function useConfigValues<K extends SettingKey>(
  keys: readonly K[],
): { [P in K]: ConfigValueType<P> } {
  return resolveConfigValues(keys, useStoredSettingValues());
}

export function resolveConfigValues<K extends SettingKey>(
  keys: readonly K[],
  stored: StoredSettingValues,
): { [P in K]: ConfigValueType<P> } {
  const result = {} as { [P in K]: ConfigValueType<P> };
  for (const key of keys) result[key] = resolveConfigValue(key, stored);
  return result;
}

export function resolveConfigValue<K extends SettingKey>(
  key: K,
  { values, hasValues }: StoredSettingValues,
): ConfigValueType<K> {
  const definition = SETTING_DEFINITIONS[key];
  const defaultValue = "default" in definition ? definition.default : undefined;

  if (
    key === "audio_retention" &&
    values.save_recordings === false &&
    !hasValues.has("audio_retention")
  ) {
    return "none" as ConfigValueType<K>;
  }

  const value = hasValues.has(key) ? values[key] : defaultValue;
  if (JSON_PARSED_KEYS.has(key)) {
    const coerceMappings = key === "personalization_dictionary_terms";
    return parseStringArray(
      value,
      parseStringArray(defaultValue, [], coerceMappings),
      coerceMappings,
    ) as ConfigValueType<K>;
  }

  return value as ConfigValueType<K>;
}

function parseStringArray(
  value: unknown,
  fallback: string[],
  coerceMappings = false,
): string[] {
  if (Array.isArray(value)) {
    return coerceStringEntries(value, coerceMappings);
  }
  if (typeof value !== "string") return fallback;
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? coerceStringEntries(parsed, coerceMappings)
      : fallback;
  } catch {
    return fallback;
  }
}

/**
 * The dictionary setting's array may hold wrong->right mapping objects next
 * to plain terms. Its config consumers want a flat string list, so a mapping
 * surfaces as its corrected (`right`) form - dropping objects would silently
 * starve STT keyword biasing of every mapped term. Scoped by `coerceMappings`
 * to the dictionary key only; every other JSON-array setting keeps the
 * strict strings-only behavior.
 */
function coerceStringEntries(
  entries: unknown[],
  coerceMappings: boolean,
): string[] {
  const coerced: string[] = [];
  for (const entry of entries) {
    if (typeof entry === "string") {
      coerced.push(entry);
    } else if (
      coerceMappings &&
      entry !== null &&
      typeof entry === "object" &&
      typeof (entry as { right?: unknown }).right === "string"
    ) {
      coerced.push((entry as { right: string }).right);
    }
  }
  return coerced;
}
