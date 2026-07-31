//! Reconhecimento de texto (CRNN + CTC) e reconstrucao da posicao das palavras.
//!
//! O modelo le a **linha inteira** de uma vez e devolve `T` passos de tempo,
//! cada um com a probabilidade de cada caractere. Como a rede so reduz a
//! largura (a altura vira 1), o passo `t` corresponde a uma faixa vertical do
//! recorte: a faixa `[t/T, (t+1)/T]` da largura.
//!
//! E dai que sai a caixa por palavra, que o produto precisa para destacar a
//! palavra sob o cursor. Nao e exato ao pixel — e a resolucao de uma faixa,
//! tipicamente 8 px do recorte — mas e o suficiente para acertar qual palavra
//! o mouse esta em cima.

use image::{imageops::FilterType, RgbImage};
use ort::session::Session;

use super::BBox;
use crate::error::{Error, Result};

/// Altura fixa que o PP-OCRv3 espera no recorte.
const ALTURA_REC: u32 = 48;
/// Teto de largura do recorte, para uma caixa esticada nao explodir a memoria.
const LARGURA_MAX: u32 = 1600;
/// Confianca minima por palavra; abaixo disso e quase sempre ruido de textura.
const CONF_MIN: f32 = 0.5;

pub struct Palavra {
    pub text: String,
    pub bbox: BBox,
    pub conf: f32,
}

pub struct Decodificado {
    pub words: Vec<Palavra>,
}

pub fn run(
    session: &mut Session,
    image: &RgbImage,
    caixa: BBox,
    charset: &[char],
) -> Result<Decodificado> {
    if caixa.w == 0 || caixa.h == 0 {
        return Ok(Decodificado { words: Vec::new() });
    }

    let recorte = image::imageops::crop_imm(image, caixa.x, caixa.y, caixa.w, caixa.h).to_image();
    let largura = ((ALTURA_REC as f32 * caixa.w as f32 / caixa.h as f32).ceil() as u32)
        .clamp(ALTURA_REC / 4, LARGURA_MAX);
    let redimensionado =
        image::imageops::resize(&recorte, largura, ALTURA_REC, FilterType::Triangle);

    let entrada = normalizar(&redimensionado);
    let (shape, saida) = super::executar(
        session,
        "x",
        [1, 3, ALTURA_REC as usize, largura as usize],
        entrada,
    )?;

    let (passos, classes) = match shape.as_slice() {
        [_, t, c] => (*t, *c),
        outro => {
            return Err(Error::Ocr(format!(
                "saida do reconhecedor com formato inesperado: {outro:?}"
            )))
        }
    };
    if classes != charset.len() {
        return Err(Error::Ocr(format!(
            "modelo tem {classes} classes mas o dicionario montou {}",
            charset.len()
        )));
    }

    Ok(Decodificado {
        words: decode(&saida, passos, classes, charset, caixa),
    })
}

/// RGB8 -> NCHW normalizado em [-1, 1], que e o pre-processamento do
/// reconhecedor (diferente do detector, que usa media/desvio do ImageNet).
fn normalizar(image: &RgbImage) -> Vec<f32> {
    let (w, h) = image.dimensions();
    let plano = (w * h) as usize;
    let mut saida = vec![0.0f32; plano * 3];
    for (i, pixel) in image.pixels().enumerate() {
        for canal in 0..3 {
            saida[canal * plano + i] = (pixel.0[canal] as f32 / 255.0 - 0.5) / 0.5;
        }
    }
    saida
}

/// Caractere emitido pelo CTC, com o passo de tempo em que saiu.
struct Emitido {
    ch: char,
    passo: usize,
    conf: f32,
}

/// Decodificacao CTC "greedy" guardando a posicao de cada caractere.
///
/// Regra do CTC: percorre os passos, pega a classe mais provavel de cada um e
/// descarta (a) a classe branca e (b) repeticoes consecutivas da mesma classe.
/// O que sobra e o texto.
pub(super) fn decode(
    saida: &[f32],
    passos: usize,
    classes: usize,
    charset: &[char],
    caixa: BBox,
) -> Vec<Palavra> {
    let mut emitidos: Vec<Emitido> = Vec::new();
    let mut anterior = usize::MAX;

    for t in 0..passos {
        let fatia = &saida[t * classes..(t + 1) * classes];
        let (melhor, &conf) = fatia
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .expect("fatia de classes nunca e vazia");

        // 0 e o branco do CTC; repeticao consecutiva e o mesmo caractere.
        if melhor != 0 && melhor != anterior {
            emitidos.push(Emitido {
                ch: charset[melhor],
                passo: t,
                conf,
            });
        }
        anterior = melhor;
    }

    agrupar_em_palavras(&emitidos, passos, caixa)
}

/// Quebra a sequencia de caracteres em palavras e converte passos de tempo em
/// coordenadas da imagem original.
fn agrupar_em_palavras(emitidos: &[Emitido], passos: usize, caixa: BBox) -> Vec<Palavra> {
    let mut palavras = Vec::new();
    let mut atual: Vec<&Emitido> = Vec::new();

    let fechar = |atual: &mut Vec<&Emitido>, palavras: &mut Vec<Palavra>| {
        if atual.is_empty() {
            return;
        }
        let texto: String = atual.iter().map(|e| e.ch).collect();
        let conf = atual.iter().map(|e| e.conf).sum::<f32>() / atual.len() as f32;
        if conf >= CONF_MIN && !texto.trim().is_empty() {
            let inicio = atual[0].passo;
            let fim = atual[atual.len() - 1].passo + 1;
            palavras.push(Palavra {
                text: texto,
                bbox: faixa_para_bbox(inicio, fim, passos, caixa),
                conf,
            });
        }
        atual.clear();
    };

    for emitido in emitidos {
        if emitido.ch == ' ' {
            fechar(&mut atual, &mut palavras);
        } else {
            atual.push(emitido);
        }
    }
    fechar(&mut atual, &mut palavras);
    palavras
}

/// Converte a faixa de passos `[inicio, fim)` na caixa correspondente dentro da
/// linha original.
fn faixa_para_bbox(inicio: usize, fim: usize, passos: usize, caixa: BBox) -> BBox {
    if passos == 0 {
        return caixa;
    }
    let por_passo = caixa.w as f32 / passos as f32;
    let x0 = (inicio as f32 * por_passo).floor().max(0.0) as u32;
    let x1 = ((fim as f32 * por_passo).ceil() as u32).min(caixa.w);
    BBox {
        x: caixa.x + x0,
        y: caixa.y,
        w: x1.saturating_sub(x0).max(1),
        h: caixa.h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta uma saida de rede falsa: `classes` por passo, com 1.0 na classe
    /// escolhida e 0.0 no resto.
    fn saida_falsa(sequencia: &[usize], classes: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; sequencia.len() * classes];
        for (t, &c) in sequencia.iter().enumerate() {
            v[t * classes + c] = 1.0;
        }
        v
    }

    /// 0 = branco, 1 = 'h', 2 = 'i', 3 = espaco.
    fn charset() -> Vec<char> {
        vec!['\u{0}', 'h', 'i', ' ']
    }

    fn caixa() -> BBox {
        BBox {
            x: 100,
            y: 50,
            w: 80,
            h: 20,
        }
    }

    #[test]
    fn ctc_descarta_branco_e_repeticoes() {
        // h h _ i i  ->  "hi"
        let seq = [1, 1, 0, 2, 2];
        let palavras = decode(&saida_falsa(&seq, 4), 5, 4, &charset(), caixa());
        assert_eq!(palavras.len(), 1);
        assert_eq!(palavras[0].text, "hi");
    }

    #[test]
    fn repeticao_separada_por_branco_vira_dois_caracteres() {
        // Sem esta regra, "hh" seria lido como "h".
        let seq = [1, 0, 1];
        let palavras = decode(&saida_falsa(&seq, 4), 3, 4, &charset(), caixa());
        assert_eq!(palavras[0].text, "hh");
    }

    #[test]
    fn espaco_separa_palavras() {
        // h _ espaco _ i
        let seq = [1, 0, 3, 0, 2];
        let palavras = decode(&saida_falsa(&seq, 4), 5, 4, &charset(), caixa());
        assert_eq!(palavras.len(), 2);
        assert_eq!(palavras[0].text, "h");
        assert_eq!(palavras[1].text, "i");
    }

    #[test]
    fn palavras_ficam_dentro_da_caixa_da_linha_e_em_ordem() {
        let seq = [1, 0, 3, 0, 2];
        let palavras = decode(&saida_falsa(&seq, 4), 5, 4, &charset(), caixa());
        let linha = caixa();
        for palavra in &palavras {
            assert!(palavra.bbox.x >= linha.x, "palavra saiu pela esquerda");
            assert!(palavra.bbox.right() <= linha.right(), "saiu pela direita");
            assert_eq!(palavra.bbox.y, linha.y);
            assert_eq!(palavra.bbox.h, linha.h);
        }
        assert!(
            palavras[0].bbox.x < palavras[1].bbox.x,
            "a primeira palavra deveria estar a esquerda da segunda"
        );
    }

    #[test]
    fn palavra_com_confianca_baixa_e_descartada() {
        let mut saida = saida_falsa(&[1, 2], 4);
        // Achata a confianca das duas emissoes para abaixo do corte.
        for v in saida.iter_mut() {
            if *v == 1.0 {
                *v = 0.2;
            }
        }
        let palavras = decode(&saida, 2, 4, &charset(), caixa());
        assert!(palavras.is_empty());
    }

    #[test]
    fn saida_so_de_branco_nao_produz_palavra() {
        let palavras = decode(&saida_falsa(&[0, 0, 0], 4), 3, 4, &charset(), caixa());
        assert!(palavras.is_empty());
    }
}
