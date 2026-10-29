import type { SafetyKey } from "./api";
import type { Tone } from "../components/Pill.svelte";

export const SAFETY_LABEL: Record<SafetyKey, string> = {
  safe: "Safe",
  review: "Review",
  protected: "Do not delete",
};

export const SAFETY_TONE: Record<SafetyKey, Tone> = {
  safe: "safe",
  review: "review",
  protected: "danger",
};
