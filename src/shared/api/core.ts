import { invoke } from "@tauri-apps/api/core";

import type {
  CardDetail,
  CardQuery,
  CardRow,
  CardSummary,
  DictEntry,
  LookupResult,
  OverlayBenchReport,
  OverlayModeChange,
  OverlayStatus,
  SaveCardInput,
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

/**
 * Salva a palavra no deck, ou anexa mais um contexto se ela ja estiver la.
 *
 * O `input.fsrs` tem que vir de `newCardFields()` — o core grava esses campos
 * sem calcular nada (regra inviolavel #4).
 */
export function deckSaveCard(input: SaveCardInput): Promise<CardSummary> {
  return invoke<CardSummary>("deck_save_card", { input });
}

/** O card de um lema, ou `null` se a palavra ainda nao esta no deck. */
export function deckCardStatus(lemma: string): Promise<CardSummary | null> {
  return invoke<CardSummary | null>("deck_card_status", { lemma });
}

/** A lista da tela Deck, ja filtrada e ordenada pelo core. */
export function deckListCards(query: CardQuery): Promise<CardRow[]> {
  return invoke<CardRow[]>("deck_list_cards", { query });
}

/** Um card com todos os contextos. `null` se ele acabou de ser excluido. */
export function deckCardDetail(cardId: number): Promise<CardDetail | null> {
  return invoke<CardDetail | null>("deck_card_detail", { cardId });
}

/** Jogos com pelo menos um contexto salvo — as opcoes do filtro. */
export function deckGames(): Promise<string[]> {
  return invoke<string[]>("deck_games");
}

/** Marca (ou desmarca) "ja sei". Nao mexe no agendamento do card. */
export function deckSetSuspended(
  cardId: number,
  suspended: boolean,
): Promise<void> {
  return invoke<void>("deck_set_suspended", { cardId, suspended });
}

/** Corrige a traducao de um contexto. `null` apaga a traducao. */
export function deckUpdateContext(
  contextId: number,
  sentencePt: string | null,
): Promise<void> {
  return invoke<void>("deck_update_context", { contextId, sentencePt });
}

/** Apaga o card, os contextos e os screenshots deles. */
export function deckDeleteCard(cardId: number): Promise<void> {
  return invoke<void>("deck_delete_card", { cardId });
}

/**
 * Bytes do screenshot de um contexto.
 *
 * Vem pela IPC, e nao por `asset://`: o caminho do banco e configuravel, entao
 * quem valida o caminho e o core (`media::resolver`).
 */
export function mediaScreenshot(path: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("media_screenshot", { path });
}

/** Executa N alternancias seguidas e devolve as estatisticas de latencia. */
export function overlayBench(iterations: number): Promise<OverlayBenchReport> {
  return invoke<OverlayBenchReport>("overlay_bench", { iterations });
}
