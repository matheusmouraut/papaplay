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
