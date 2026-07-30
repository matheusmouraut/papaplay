import { invoke } from "@tauri-apps/api/core";

import type {
  OverlayBenchReport,
  OverlayModeChange,
  OverlayStatus,
} from "../types";

/**
 * Ponte tipada com os comandos do core Rust.
 * Cada comando novo em `src-tauri/src/` ganha um wrapper aqui.
 */

/** Health-check do core — usado pelas telas placeholder do bootstrap. */
export function ping(): Promise<string> {
  return invoke<string>("ping");
}

/** Entra (`true`) ou sai (`false`) do modo lookup. */
export function overlaySetMode(
  interactive: boolean,
): Promise<OverlayModeChange> {
  return invoke<OverlayModeChange>("overlay_set_mode", { interactive });
}

/** Inverte o modo atual — mesmo caminho da hotkey `Alt+X`. */
export function overlayToggle(): Promise<OverlayModeChange> {
  return invoke<OverlayModeChange>("overlay_toggle");
}

export function overlayStatus(): Promise<OverlayStatus> {
  return invoke<OverlayStatus>("overlay_status");
}

/** Executa N alternancias seguidas e devolve as estatisticas de latencia. */
export function overlayBench(iterations: number): Promise<OverlayBenchReport> {
  return invoke<OverlayBenchReport>("overlay_bench", { iterations });
}
