import { invoke } from "@tauri-apps/api/core";

import type {
  DictEntry,
  LookupResult,
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

/**
 * Captura o entorno do cursor e roda o OCR, devolvendo as palavras ja
 * posicionadas em coordenadas da overlay.
 *
 * Bloqueia por centenas de milissegundos: a primeira chamada ainda carrega os
 * modelos ONNX (~150 ms a mais).
 */
export function lookupRun(): Promise<LookupResult> {
  return invoke<LookupResult>("lookup_run");
}

/**
 * Busca uma palavra no dicionario offline, lematizando antes.
 *
 * Aceita a palavra como o OCR entregou — pontuacao e caixa sao normalizadas no
 * core. `null` quando o dicionario nao conhece a palavra.
 */
export function dictLookup(word: string): Promise<DictEntry | null> {
  return invoke<DictEntry | null>("dict_lookup", { word });
}

/**
 * Traduz uma frase de ingles para portugues do Brasil, offline.
 *
 * Bloqueia por dezenas de milissegundos por frase, mais ~800 ms na primeira
 * chamada de cada sessao de lookup — que e quando o modelo entra na memoria.
 * Ele sai dela ao voltar para o modo passivo (`translate::unload_model`), entao
 * essa carga reaparece a cada Alt+X.
 */
export function translateRun(text: string): Promise<string> {
  return invoke<string>("translate_run", { text });
}

/** Executa N alternancias seguidas e devolve as estatisticas de latencia. */
export function overlayBench(iterations: number): Promise<OverlayBenchReport> {
  return invoke<OverlayBenchReport>("overlay_bench", { iterations });
}
