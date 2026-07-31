//! Deteccao de texto (DBNet).
//!
//! A rede devolve um mapa de probabilidade do tamanho da imagem reduzida. O
//! trabalho aqui e transformar esse mapa em caixas:
//!
//! 1. binariza pelo limiar `thresh`;
//! 2. acha as regioes conectadas (cada mancha = uma linha de texto);
//! 3. pontua cada regiao pela probabilidade media e descarta as fracas;
//! 4. expande a caixa (`unclip`), porque o DBNet e treinado para prever o
//!    "miolo" do texto e nao a borda;
//! 5. converte de volta para as coordenadas da imagem original.

use image::{imageops::FilterType, RgbImage};
use ort::session::Session;

use super::BBox;
use crate::error::{Error, Result};

/// Ajustes do detector. Os defaults sao os do PaddleOCR, com `box_thresh` um
/// pouco mais baixo: texto de jogo costuma ter borda/sombra, o que derruba a
/// probabilidade media da mancha.
#[derive(Debug, Clone, Copy)]
pub struct DetectParams {
    /// Maior lado da imagem entregue a rede. Controla custo x acuidade.
    pub limit_side_len: u32,
    /// Limiar de binarizacao do mapa de probabilidade.
    pub thresh: f32,
    /// Probabilidade media minima para a mancha virar caixa.
    pub box_thresh: f32,
    /// Quanto a caixa cresce depois de detectada.
    pub unclip_ratio: f32,
    /// Caixas com lado menor que isto (em pixels da imagem original) somem.
    pub min_size: u32,
    /// Sobreposicao vertical minima para duas caixas serem a mesma linha.
    pub line_overlap: f32,
    /// Espaco horizontal maximo entre caixas da mesma linha, em multiplos da
    /// altura da caixa. Cobre o caso de um icone partindo a frase em duas.
    pub line_gap_ratio: f32,
}

impl Default for DetectParams {
    fn default() -> Self {
        Self {
            limit_side_len: 960,
            thresh: 0.3,
            box_thresh: 0.5,
            unclip_ratio: 1.6,
            min_size: 3,
            line_overlap: 0.5,
            line_gap_ratio: 1.2,
        }
    }
}

/// Media e desvio do ImageNet, que e como os modelos PP-OCR foram treinados.
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

pub fn run(session: &mut Session, image: &RgbImage, params: &DetectParams) -> Result<Vec<BBox>> {
    let (orig_w, orig_h) = image.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return Ok(Vec::new());
    }

    let (rede_w, rede_h) = tamanho_da_rede(orig_w, orig_h, params.limit_side_len);
    let reduzida = image::imageops::resize(image, rede_w, rede_h, FilterType::Triangle);

    let entrada = normalizar(&reduzida);
    let (shape, mapa) = super::executar(
        session,
        "x",
        [1, 3, rede_h as usize, rede_w as usize],
        entrada,
    )?;

    // [N, 1, H, W]: so a altura e a largura importam aqui.
    let (mapa_h, mapa_w) = match shape.as_slice() {
        [_, _, h, w] => (*h, *w),
        outro => {
            return Err(Error::Ocr(format!(
                "saida do detector com formato inesperado: {outro:?}"
            )))
        }
    };

    let escala_x = orig_w as f32 / mapa_w as f32;
    let escala_y = orig_h as f32 / mapa_h as f32;

    let mut caixas = Vec::new();
    for regiao in regioes(&mapa, mapa_w, mapa_h, params.thresh) {
        if regiao.score(&mapa, mapa_w) < params.box_thresh {
            continue;
        }
        if let Some(caixa) = regiao.para_bbox(params, escala_x, escala_y, orig_w, orig_h) {
            caixas.push(caixa);
        }
    }

    // De cima para baixo, da esquerda para a direita: e a ordem de leitura, e
    // deixa o agrupamento em linhas determinístico.
    caixas.sort_by(|a, b| {
        a.y.cmp(&b.y)
            .then(a.x.cmp(&b.x))
            .then(a.w.cmp(&b.w))
            .then(a.h.cmp(&b.h))
    });
    Ok(caixas)
}

/// Reduz a imagem ate `limit` no maior lado e arredonda os dois lados para
/// multiplos de 32 — exigencia da arquitetura do DBNet.
fn tamanho_da_rede(w: u32, h: u32, limit: u32) -> (u32, u32) {
    let maior = w.max(h) as f32;
    let fator = if maior > limit as f32 {
        limit as f32 / maior
    } else {
        1.0
    };
    let ajustar = |v: f32| ((v / 32.0).round().max(1.0) as u32) * 32;
    (ajustar(w as f32 * fator), ajustar(h as f32 * fator))
}

/// RGB8 -> NCHW float normalizado.
fn normalizar(image: &RgbImage) -> Vec<f32> {
    let (w, h) = image.dimensions();
    let plano = (w * h) as usize;
    let mut saida = vec![0.0f32; plano * 3];
    for (i, pixel) in image.pixels().enumerate() {
        for canal in 0..3 {
            saida[canal * plano + i] = (pixel.0[canal] as f32 / 255.0 - MEAN[canal]) / STD[canal];
        }
    }
    saida
}

/// Mancha conectada no mapa binarizado.
struct Regiao {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    pixels: Vec<usize>,
}

impl Regiao {
    /// Probabilidade media dentro da mancha. E o que separa texto de ruido:
    /// uma mancha grande e fraca (textura, borda de HUD) fica abaixo do corte.
    fn score(&self, mapa: &[f32], _largura: usize) -> f32 {
        if self.pixels.is_empty() {
            return 0.0;
        }
        let soma: f32 = self.pixels.iter().map(|&i| mapa[i]).sum();
        soma / self.pixels.len() as f32
    }

    fn para_bbox(
        &self,
        params: &DetectParams,
        escala_x: f32,
        escala_y: f32,
        limite_w: u32,
        limite_h: u32,
    ) -> Option<BBox> {
        let w = (self.x1 - self.x0 + 1) as f32;
        let h = (self.y1 - self.y0 + 1) as f32;

        // O DBNet encolhe o poligono no treino; `unclip` desfaz isso. A
        // distancia vem da razao area/perimetro, como no PaddleOCR.
        let distancia = (w * h * params.unclip_ratio) / (2.0 * (w + h));

        let x = (self.x0 as f32 - distancia) * escala_x;
        let y = (self.y0 as f32 - distancia) * escala_y;
        let largura = (w + 2.0 * distancia) * escala_x;
        let altura = (h + 2.0 * distancia) * escala_y;

        let x = x.max(0.0).round() as u32;
        let y = y.max(0.0).round() as u32;
        let largura = (largura.round() as u32).min(limite_w.saturating_sub(x));
        let altura = (altura.round() as u32).min(limite_h.saturating_sub(y));

        if largura < params.min_size || altura < params.min_size {
            return None;
        }
        Some(BBox {
            x,
            y,
            w: largura,
            h: altura,
        })
    }
}

/// Rotulacao de componentes conectados (8-vizinhos), iterativa.
///
/// Iterativa e nao recursiva de proposito: uma mancha de texto numa tela 2.5K
/// chega a dezenas de milhares de pixels e a versao recursiva estoura a pilha.
fn regioes(mapa: &[f32], largura: usize, altura: usize, thresh: f32) -> Vec<Regiao> {
    let mut visitado = vec![false; largura * altura];
    let mut saida = Vec::new();
    let mut pilha = Vec::new();

    for inicio in 0..largura * altura {
        if visitado[inicio] || mapa[inicio] <= thresh {
            continue;
        }
        visitado[inicio] = true;
        pilha.push(inicio);

        let mut regiao = Regiao {
            x0: inicio % largura,
            y0: inicio / largura,
            x1: inicio % largura,
            y1: inicio / largura,
            pixels: Vec::new(),
        };

        while let Some(atual) = pilha.pop() {
            let cx = atual % largura;
            let cy = atual / largura;
            regiao.x0 = regiao.x0.min(cx);
            regiao.x1 = regiao.x1.max(cx);
            regiao.y0 = regiao.y0.min(cy);
            regiao.y1 = regiao.y1.max(cy);
            regiao.pixels.push(atual);

            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = cx as i64 + dx;
                    let ny = cy as i64 + dy;
                    if nx < 0 || ny < 0 || nx >= largura as i64 || ny >= altura as i64 {
                        continue;
                    }
                    let vizinho = ny as usize * largura + nx as usize;
                    if !visitado[vizinho] && mapa[vizinho] > thresh {
                        visitado[vizinho] = true;
                        pilha.push(vizinho);
                    }
                }
            }
        }
        saida.push(regiao);
    }
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamanho_da_rede_e_multiplo_de_32() {
        let (w, h) = tamanho_da_rede(2560, 1440, 960);
        assert_eq!(w % 32, 0);
        assert_eq!(h % 32, 0);
        assert!(w <= 960 + 32);
    }

    #[test]
    fn imagem_menor_que_o_limite_nao_e_ampliada() {
        let (w, h) = tamanho_da_rede(320, 200, 960);
        assert!(w <= 320 + 32);
        assert!(h <= 200 + 32);
    }

    #[test]
    fn regioes_separa_duas_manchas_distantes() {
        // 8x2: duas manchas de 2 pixels, separadas por uma coluna apagada.
        let mapa = vec![
            0.9, 0.9, 0.0, 0.0, 0.9, 0.9, 0.0, 0.0, //
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let achadas = regioes(&mapa, 8, 2, 0.3);
        assert_eq!(achadas.len(), 2);
        assert_eq!(achadas[0].x0, 0);
        assert_eq!(achadas[0].x1, 1);
        assert_eq!(achadas[1].x0, 4);
        assert_eq!(achadas[1].x1, 5);
    }

    #[test]
    fn regioes_une_pixels_na_diagonal() {
        // 8-vizinhos: a diagonal conecta, entao isto e UMA mancha.
        let mapa = vec![
            0.9, 0.0, //
            0.0, 0.9,
        ];
        assert_eq!(regioes(&mapa, 2, 2, 0.3).len(), 1);
    }

    #[test]
    fn score_e_a_media_da_probabilidade_da_mancha() {
        let mapa = vec![0.4, 0.8, 0.0, 0.0];
        let regiao = Regiao {
            x0: 0,
            y0: 0,
            x1: 1,
            y1: 0,
            pixels: vec![0, 1],
        };
        assert!((regiao.score(&mapa, 2) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn unclip_expande_a_caixa_detectada() {
        let params = DetectParams::default();
        let regiao = Regiao {
            x0: 10,
            y0: 10,
            x1: 29,
            y1: 19,
            pixels: vec![0],
        };
        let caixa = regiao.para_bbox(&params, 1.0, 1.0, 1000, 1000).unwrap();
        assert!(caixa.w > 20, "largura deveria crescer, veio {}", caixa.w);
        assert!(caixa.x < 10, "x deveria recuar, veio {}", caixa.x);
    }

    #[test]
    fn caixa_nao_escapa_dos_limites_da_imagem() {
        let params = DetectParams::default();
        let regiao = Regiao {
            x0: 0,
            y0: 0,
            x1: 99,
            y1: 19,
            pixels: vec![0],
        };
        let caixa = regiao.para_bbox(&params, 1.0, 1.0, 100, 20).unwrap();
        assert!(caixa.right() <= 100);
        assert!(caixa.bottom() <= 20);
    }
}
