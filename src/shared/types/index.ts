/** Tipos compartilhados entre as duas janelas e o core Rust. */

/** Retangulo em coordenadas de tela fisicas (pixels do monitor capturado). */
export interface BBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Palavra reconhecida pelo OCR. */
export interface OcrWord {
  text: string;
  bbox: BBox;
  conf: number;
  /** Indice da linha a qual a palavra pertence, em `OcrResult.lines`. */
  lineIndex: number;
}

/** Linha de texto reconstruida a partir das palavras. */
export interface OcrLine {
  text: string;
  bbox: BBox;
}

export interface OcrResult {
  words: OcrWord[];
  lines: OcrLine[];
  /** Titulo da janela em foco no momento da captura (nome do jogo). */
  gameName: string | null;
  capturedAt: string;
}

/** Retangulo de um monitor em pixels fisicos da area de trabalho virtual. */
export interface MonitorRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Resultado de uma troca de modo da overlay (evento `overlay://mode`). */
export interface OverlayModeChange {
  /** `true` = modo lookup (recebe cliques); `false` = passivo (click-through). */
  interactive: boolean;
  /** Duracao da troca medida no core, em microssegundos. */
  elapsedUs: number;
  /** Titulo da janela em foco ao entrar em lookup. */
  windowTitle: string | null;
  monitor: MonitorRect | null;
  scaleFactor: number;
}

export interface OverlayStatus {
  interactive: boolean;
  scaleFactor: number;
}

/** Estatisticas do benchmark de alternancia (evento `overlay://bench`). */
export interface OverlayBenchReport {
  iterations: number;
  minUs: number;
  maxUs: number;
  meanUs: number;
  p50Us: number;
  p95Us: number;
  failures: number;
  samplesUs: number[];
}

/** Uma acepcao do dicionario. */
export interface Sense {
  pos: string;
  glossPt: string;
  glossEn?: string;
  examples?: string[];
}

export interface DictEntry {
  lemma: string;
  ipa: string | null;
  senses: Sense[];
  freqRank: number | null;
}

export type FsrsState = "new" | "learning" | "review" | "relearning";

/** Card do deck. Os campos `fsrs*` so devem ser alterados via `src/shared/srs`. */
export interface DeckCard {
  id: number;
  lemma: string;
  createdAt: string;
  suspended: boolean;
  fsrsDue: string;
  fsrsStability: number;
  fsrsDifficulty: number;
  fsrsState: FsrsState;
  fsrsReps: number;
  fsrsLapses: number;
}

/** Ocorrencia da palavra num jogo, anexada a um card. */
export interface CardContext {
  id: number;
  cardId: number;
  form: string;
  sentenceEn: string;
  sentencePt: string | null;
  gameName: string | null;
  screenshotPath: string | null;
  capturedAt: string;
}
