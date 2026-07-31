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

/** Acepcao de uma palavra no dicionario offline. */
export interface DictSense {
  /** Classe gramatical crua do Wiktionary ("noun", "verb", "adj"...). */
  pos: string;
  /** Traducoes/definicao em portugues — e o que o tooltip mostra. */
  glossPt: string;
  /** Definicao em ingles, quando a fonte tinha uma. */
  glossEn: string | null;
  examples: string[];
}

/** Verbete devolvido por `dict_lookup`. */
export interface DictEntry {
  lemma: string;
  ipa: string | null;
  senses: DictSense[];
  /** Posicao na lista de frequencia; menor = mais comum. */
  freqRank: number | null;
  /** A forma que estava na tela, quando difere do lema ("ran" para "run"). */
  matchedForm: string | null;
}

/**
 * Retangulo em pixels **logicos**, relativo ao canto da overlay.
 *
 * Ja vem convertido do core: a overlay posiciona direto, sem tocar em escala
 * de DPI. A conversao mora em `lookup::para_overlay` no Rust.
 */
export interface OverlayRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Palavra reconhecida, ja posicionada para desenhar na overlay. */
export interface LookupWord {
  text: string;
  rect: OverlayRect;
  conf: number;
  /** Indice da linha em `LookupResult.lines` — de onde sai a frase do card. */
  lineIndex: number;
}

export interface LookupLine {
  text: string;
  rect: OverlayRect;
}

/** Resultado de uma consulta: captura + OCR de uma janela em volta do cursor. */
export interface LookupResult {
  words: LookupWord[];
  lines: LookupLine[];
  /** Cursor em pixels logicos da overlay no instante da captura. */
  cursor: [number, number] | null;
  gameName: string | null;
  /** Recorte lido, em pixels fisicos — diagnostico. */
  region: { x: number; y: number; width: number; height: number };
  captureMs: number;
  ocrMs: number;
  totalMs: number;
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
