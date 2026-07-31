//! OCR via modelos PP-OCR (RapidOCR) em ONNX, rodando no crate `ort`.
//!
//! Pipeline, em duas passadas de rede neural:
//!
//! 1. **Deteccao** (DBNet) — recebe a tela inteira reduzida e devolve um mapa
//!    de probabilidade "aqui tem texto". Dele saem as caixas de cada linha.
//! 2. **Reconhecimento** (CRNN + CTC) — recebe o recorte de cada linha e
//!    devolve a sequencia de caracteres.
//!
//! As caixas **por palavra** nao vem de graca: o PP-OCR reconhece a linha
//! inteira de uma vez. A posicao de cada palavra e reconstruida a partir do
//! passo de tempo do CTC em que cada caractere foi emitido — ver
//! [`recognize::decode`].
//!
//! Plano B documentado em docs/04: Windows.Media.Ocr.
//! Alvo de performance: lookup completo (captura + OCR + dict) < 1s.

mod detect;
mod lines;
mod recognize;

use std::path::Path;

use image::RgbImage;
use ort::session::Session;
use serde::Serialize;

use crate::capture::Frame;
use crate::error::{Error, Result};

pub use detect::DetectParams;

/// Retangulo em pixels fisicos do frame capturado.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl BBox {
    /// Borda direita (exclusiva).
    pub fn right(&self) -> u32 {
        self.x + self.w
    }

    /// Borda inferior (exclusiva).
    pub fn bottom(&self) -> u32 {
        self.y + self.h
    }

    /// Centro vertical — base do agrupamento em linhas.
    pub fn center_y(&self) -> f32 {
        self.y as f32 + self.h as f32 / 2.0
    }

    /// Fracao da altura que as duas caixas compartilham (0.0 a 1.0).
    ///
    /// Usa a **menor** das duas alturas como denominador: assim uma caixa
    /// baixa contida numa alta conta como sobreposicao total, que e o que
    /// interessa para decidir se estao na mesma linha.
    pub fn vertical_overlap(&self, other: &BBox) -> f32 {
        let topo = self.y.max(other.y);
        let base = self.bottom().min(other.bottom());
        if base <= topo {
            return 0.0;
        }
        let comum = (base - topo) as f32;
        comum / self.h.min(other.h).max(1) as f32
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrWord {
    pub text: String,
    pub bbox: BBox,
    pub conf: f32,
    /// Indice da linha correspondente em [`OcrResult::lines`].
    pub line_index: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct OcrLine {
    pub text: String,
    pub bbox: BBox,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub words: Vec<OcrWord>,
    pub lines: Vec<OcrLine>,
}

/// Modelos carregados. Criar e caro (le ~11 MB de ONNX e inicializa o runtime);
/// reconhecer e barato. Manter uma instancia viva pelo processo todo.
pub struct Engine {
    det: Session,
    rec: Session,
    /// Indexado pela classe de saida do reconhecedor: 0 = branco do CTC,
    /// 1..=95 = caracteres do dicionario, 96 = espaco.
    charset: Vec<char>,
    params: DetectParams,
}

impl Engine {
    /// Carrega os modelos de um diretorio com `en_PP-OCRv3_det_infer.onnx`,
    /// `en_PP-OCRv3_rec_infer.onnx` e `en_dict.txt`.
    pub fn load(models_dir: &Path) -> Result<Self> {
        Self::load_with(models_dir, DetectParams::default())
    }

    pub fn load_with(models_dir: &Path, params: DetectParams) -> Result<Self> {
        let det = sessao(&models_dir.join("en_PP-OCRv3_det_infer.onnx"))?;
        let rec = sessao(&models_dir.join("en_PP-OCRv3_rec_infer.onnx"))?;
        let charset = charset(&models_dir.join("en_dict.txt"))?;
        Ok(Self {
            det,
            rec,
            charset,
            params,
        })
    }

    pub fn params(&self) -> &DetectParams {
        &self.params
    }

    pub fn set_params(&mut self, params: DetectParams) {
        self.params = params;
    }

    /// So a deteccao — util para inspecionar caixas sem pagar o reconhecimento.
    pub fn detect(&mut self, image: &RgbImage) -> Result<Vec<BBox>> {
        detect::run(&mut self.det, image, &self.params)
    }

    /// Pipeline completo sobre uma imagem RGB.
    pub fn recognize_image(&mut self, image: &RgbImage) -> Result<OcrResult> {
        let caixas = self.detect(image)?;
        let agrupadas = lines::group(caixas, &self.params);

        let mut words = Vec::new();
        let mut out_lines = Vec::new();

        for (line_index, grupo) in agrupadas.iter().enumerate() {
            let mut texto_linha = String::new();

            for caixa in grupo {
                let decodificado = recognize::run(&mut self.rec, image, *caixa, &self.charset)?;
                for palavra in decodificado.words {
                    if !texto_linha.is_empty() {
                        texto_linha.push(' ');
                    }
                    texto_linha.push_str(&palavra.text);
                    words.push(OcrWord {
                        text: palavra.text,
                        bbox: palavra.bbox,
                        conf: palavra.conf,
                        line_index,
                    });
                }
            }

            if texto_linha.is_empty() {
                continue;
            }
            out_lines.push(OcrLine {
                text: texto_linha,
                bbox: lines::envelope(grupo),
            });
        }

        // `line_index` aponta para `out_lines`, que pula grupos vazios — sem
        // este remapeamento os indices ficariam furados.
        reindexar(&mut words, &agrupadas, &out_lines);

        Ok(OcrResult {
            words,
            lines: out_lines,
        })
    }
}

/// Corrige `line_index` depois que grupos sem texto foram descartados.
fn reindexar(words: &mut [OcrWord], agrupadas: &[Vec<BBox>], out_lines: &[OcrLine]) {
    if agrupadas.len() == out_lines.len() {
        return;
    }
    let mut mapa = vec![usize::MAX; agrupadas.len()];
    let mut destino = 0usize;
    for (origem, grupo) in agrupadas.iter().enumerate() {
        let envelope = lines::envelope(grupo);
        if destino < out_lines.len() && out_lines[destino].bbox == envelope {
            mapa[origem] = destino;
            destino += 1;
        }
    }
    for palavra in words {
        if let Some(&novo) = mapa.get(palavra.line_index) {
            if novo != usize::MAX {
                palavra.line_index = novo;
            }
        }
    }
}

/// Executa um modelo de uma entrada e uma saida, devolvendo `(formato, dados)`.
///
/// Concentra aqui tudo que depende da API do `ort`: como ela ainda esta em
/// release candidate e muda entre versoes, um unico ponto de contato mantem o
/// resto do modulo estavel.
fn executar(
    session: &mut Session,
    entrada: &str,
    shape: [usize; 4],
    dados: Vec<f32>,
) -> Result<(Vec<usize>, Vec<f32>)> {
    let esperado: usize = shape.iter().product();
    if dados.len() != esperado {
        return Err(Error::Ocr(format!(
            "tensor com {} valores para o formato {shape:?} (esperado {esperado})",
            dados.len()
        )));
    }

    let tensor = ort::value::Tensor::from_array((shape, dados))
        .map_err(|e| Error::Ocr(format!("tensor de entrada invalido: {e}")))?;

    let saidas = session
        .run(ort::inputs![entrada => tensor])
        .map_err(|e| Error::Ocr(format!("inferencia falhou: {e}")))?;

    let (_, valor) = saidas
        .into_iter()
        .next()
        .ok_or_else(|| Error::Ocr("modelo nao devolveu saida".into()))?;

    let (formato, plano) = valor
        .try_extract_tensor::<f32>()
        .map_err(|e| Error::Ocr(format!("saida ilegivel: {e}")))?;

    Ok((
        formato.iter().map(|&d| d.max(0) as usize).collect(),
        plano.to_vec(),
    ))
}

/// Teto de threads que o ONNX Runtime usa **dentro** de uma inferencia.
///
/// Medido na spike 02 (i7-12700H, 20 threads, maquina ociosa, 10 fixtures 2.5K):
///
/// | threads | media  | pior    |
/// |---------|--------|---------|
/// | 1       | 855 ms | 1931 ms |
/// | 2       | 456 ms | 1020 ms |
/// | **4**   | 323 ms |  732 ms |
/// | 8       | 321 ms |  725 ms |
/// | 14      | 356 ms |  793 ms |
///
/// O ganho satura em 4 e volta a piorar em 14 (disputa entre threads). Como
/// isto roda **por cima de um jogo**, 4 entrega praticamente todo o ganho sem
/// tomar a maquina de quem esta jogando — o default do runtime (todos os
/// nucleos) seria pior nos dois sentidos.
///
/// Paralelizar por fora nao e opcao: `Session::run` exige acesso exclusivo,
/// entao seria preciso carregar o modelo uma vez por thread, e a RAM ja esta
/// apertada (spike 01, problema 1).
const THREADS_MAX: usize = 4;

/// `PAPAPLAY_OCR_THREADS` existe para repetir a varredura acima.
fn intra_threads() -> usize {
    if let Some(forcado) = std::env::var("PAPAPLAY_OCR_THREADS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
    {
        return forcado;
    }
    std::thread::available_parallelism()
        .map(|n| n.get().min(THREADS_MAX))
        .unwrap_or(1)
}

fn sessao(caminho: &Path) -> Result<Session> {
    let construir = || -> std::result::Result<Session, ort::Error> {
        let mut builder = Session::builder()?.with_intra_threads(intra_threads())?;
        builder.commit_from_file(caminho)
    };

    construir().map_err(|e| Error::Ocr(format!("falha ao carregar {}: {e}", caminho.display())))
}

/// Monta a tabela de classes do reconhecedor.
///
/// A convencao do PaddleOCR e `[branco] + dicionario + [espaco]`: por isso o
/// modelo tem 97 saidas para um dicionario de 95 caracteres.
fn charset(caminho: &Path) -> Result<Vec<char>> {
    let bruto = std::fs::read_to_string(caminho)?;
    let mut tabela = Vec::with_capacity(97);
    tabela.push('\u{0}'); // branco do CTC
    for linha in bruto.lines() {
        let mut chars = linha.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => tabela.push(c),
            (None, _) => continue, // linha vazia no fim do arquivo
            _ => {
                return Err(Error::Ocr(format!(
                    "linha do dicionario com mais de um caractere: {linha:?}"
                )))
            }
        }
    }
    tabela.push(' ');
    Ok(tabela)
}

/// Roda deteccao + reconhecimento sobre o frame, devolvendo bboxes por palavra.
pub fn recognize(engine: &mut Engine, frame: &Frame) -> Result<OcrResult> {
    engine.recognize_image(&frame_para_rgb(frame)?)
}

/// BGRA8 (formato da captura do Windows) para RGB8 (formato dos modelos).
fn frame_para_rgb(frame: &Frame) -> Result<RgbImage> {
    let esperado = frame.width as usize * frame.height as usize * 4;
    if frame.pixels.len() < esperado {
        return Err(Error::Ocr(format!(
            "frame truncado: {} bytes para {}x{}",
            frame.pixels.len(),
            frame.width,
            frame.height
        )));
    }
    let mut rgb = RgbImage::new(frame.width, frame.height);
    for (i, pixel) in rgb.pixels_mut().enumerate() {
        let b = frame.pixels[i * 4];
        let g = frame.pixels[i * 4 + 1];
        let r = frame.pixels[i * 4 + 2];
        *pixel = image::Rgb([r, g, b]);
    }
    Ok(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(x: u32, y: u32, w: u32, h: u32) -> BBox {
        BBox { x, y, w, h }
    }

    #[test]
    fn sobreposicao_vertical_total_de_caixa_contida() {
        let alta = bbox(0, 0, 10, 40);
        let baixa = bbox(20, 10, 10, 20);
        assert_eq!(alta.vertical_overlap(&baixa), 1.0);
    }

    #[test]
    fn sobreposicao_vertical_zero_para_caixas_separadas() {
        let a = bbox(0, 0, 10, 10);
        let b = bbox(0, 30, 10, 10);
        assert_eq!(a.vertical_overlap(&b), 0.0);
    }

    #[test]
    fn frame_bgra_vira_rgb_com_canais_trocados() {
        let frame = Frame {
            width: 1,
            height: 1,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            window_title: None,
            pixels: vec![10, 20, 30, 255], // B, G, R, A
        };
        let rgb = frame_para_rgb(&frame).expect("conversao");
        assert_eq!(rgb.get_pixel(0, 0).0, [30, 20, 10]);
    }

    #[test]
    fn frame_truncado_e_erro_em_vez_de_panico() {
        let frame = Frame {
            width: 4,
            height: 4,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            window_title: None,
            pixels: vec![0; 8],
        };
        assert!(frame_para_rgb(&frame).is_err());
    }
}
