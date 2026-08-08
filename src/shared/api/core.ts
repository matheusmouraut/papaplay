import { invoke } from "@tauri-apps/api/core";

import type {
  CardDetail,
  CardQuery,
  CardRow,
  CardSummary,
  DictEntry,
  LookupResult,
  NmtStatus,
  OverlayBenchReport,
  OverlayModeChange,
  OverlayStatus,
  PeekState,
  Preferences,
  QueueQuery,
  ReviewInput,
  ReviewQueue,
  SaveCardInput,
  Shortcuts,
  StatsQuery,
  StatsSummary,
} from "../types";

/**
 * Ponte tipada com os comandos do core Rust.
 * Cada comando novo em `src-tauri/src/` ganha um wrapper aqui.
 */

/** Health-check do core — usado pelas telas placeholder do bootstrap. */
export function ping(): Promise<string> {
  return invoke<string>("ping");
}

/**
 * Faz a overlay receber (`true`) ou ignorar (`false`) cliques.
 *
 * Quem chama e o core, ao abrir e fechar o card — a UI nao decide isso, porque
 * espiando ela nem recebe mouse. Fica exposto para diagnostico.
 */
export function overlaySetMode(
  interactive: boolean,
): Promise<OverlayModeChange> {
  return invoke<OverlayModeChange>("overlay_set_mode", { interactive });
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

/** Fecha o card e volta ao repouso. E o que o clique fora chama. */
export function peekClose(): Promise<void> {
  return invoke<void>("peek_close");
}

/** Estado da espiada agora — usado so para a UI se sincronizar ao montar. */
export function peekState(): Promise<PeekState> {
  return invoke<PeekState>("peek_state");
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

/**
 * Executa N alternancias seguidas e devolve as estatisticas de latencia.
 *
 * Sobrou da spike 01. Nao ha UI para ele desde que a overlay virou "espiar";
 * fica para medir regressao de latencia pelo console.
 */
export function overlayBench(iterations: number): Promise<OverlayBenchReport> {
  return invoke<OverlayBenchReport>("overlay_bench", { iterations });
}

/** Os atalhos salvos agora, ou os padrões na primeira execução. */
export function settingsGetShortcuts(): Promise<Shortcuts> {
  return invoke<Shortcuts>("settings_get_shortcuts");
}

/**
 * Valida, persiste e re-registra os atalhos globais — sem reiniciar o app.
 *
 * Rejeita combinações malformadas e combinações repetidas entre `lookup` e
 * `card` antes mesmo de chamar o Windows; uma combinação já tomada por outro
 * app só falha aqui dentro, no `RegisterHotKey` (core, `hotkeys::reregister`).
 */
export function settingsSetShortcuts(shortcuts: Shortcuts): Promise<Shortcuts> {
  return invoke<Shortcuts>("settings_set_shortcuts", { shortcuts });
}

/** Volta `lookup`/`card` para Alt+X / Alt+C. */
export function settingsResetShortcuts(): Promise<Shortcuts> {
  return invoke<Shortcuts>("settings_reset_shortcuts");
}

/** O tradutor de frases já está instalado nesta máquina? */
export function nmtStatus(): Promise<NmtStatus> {
  return invoke<NmtStatus>("nmt_status");
}

/**
 * Baixa o tradutor de frases (~332 MB), emitindo `setup://nmt` a cada pedaço.
 *
 * É a única chamada de rede do app, e ela só acontece por ação explícita do
 * usuário no setup — o resto funciona com o cabo desconectado.
 */
export function nmtInstall(): Promise<NmtStatus> {
  return invoke<NmtStatus>("nmt_install");
}

/** Preferências salvas, ou os padrões na primeira execução. */
export function settingsGetPreferences(): Promise<Preferences> {
  return invoke<Preferences>("settings_get_preferences");
}

/** Grava e devolve o que ficou salvo — `newPerDay` volta já limitado a 1..200. */
export function settingsSetPreferences(
  preferences: Preferences,
): Promise<Preferences> {
  return invoke<Preferences>("settings_set_preferences", { preferences });
}

/**
 * A fila do dia: cards vencidos + a cota de novos que ainda sobrou.
 *
 * O `now`/`dayStart` vêm daqui, e não do core, porque só a UI sabe o fuso do
 * usuário — "novos por dia" é uma pergunta sobre o dia local.
 */
export function reviewQueue(query: QueueQuery): Promise<ReviewQueue> {
  return invoke<ReviewQueue>("review_queue", { query });
}

/**
 * Grava a nota: novo agendamento no card e uma linha no histórico.
 *
 * O `input.fsrs` tem que vir do wrapper em `src/shared/srs` — o core persiste
 * sem calcular nada (regra inviolável #4).
 */
export function reviewApply(input: ReviewInput): Promise<void> {
  return invoke<void>("review_apply", { input });
}

/** Números da tela de Estatísticas, já agregados pelo core. */
export function statsSummary(query: StatsQuery): Promise<StatsSummary> {
  return invoke<StatsSummary>("stats_summary", { query });
}

/**
 * Escreve o CSV do deck no caminho escolhido e devolve quantas linhas gravou.
 *
 * Uma linha por contexto, não por card: a frase onde a palavra apareceu é o que
 * este deck tem que uma lista de vocabulário não tem.
 */
export function deckExportCsv(path: string): Promise<number> {
  return invoke<number>("deck_export_csv", { path });
}
