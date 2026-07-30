//! OCR via RapidOCR (modelos ONNX) rodando no crate `ort`.
//!
//! Plano B documentado em docs/04: Windows.Media.Ocr.
//! Alvo de performance: lookup completo (captura + OCR + dict) < 1s.

use serde::Serialize;

use crate::capture::Frame;
use crate::error::{Error, Result};

/// Retangulo em pixels fisicos do frame capturado.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrWord {
    pub text: String,
    pub bbox: BBox,
    pub conf: f32,
    /// Indice da linha correspondente em [`OcrResult::lines`].
    pub line_index: usize,
}

#[derive(Debug, Serialize)]
pub struct OcrLine {
    pub text: String,
    pub bbox: BBox,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub words: Vec<OcrWord>,
    pub lines: Vec<OcrLine>,
}

/// Roda deteccao + reconhecimento sobre o frame, devolvendo bboxes por palavra.
pub fn recognize(_frame: &Frame) -> Result<OcrResult> {
    Err(Error::NotImplemented("ocr::recognize"))
}
