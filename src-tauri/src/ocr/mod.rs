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
pub mod sentences;

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

    /// Centro horizontal — usado para reconhecer texto centralizado, que e
    /// como quase todo jogo desenha legenda e dialogo.
    pub fn center_x(&self) -> f32 {
        self.x as f32 + self.w as f32 / 2.0
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

    /// Pipeline completo sobre uma imagem RGB: le **tudo** que houver nela.
    pub fn recognize_image(&mut self, image: &RgbImage) -> Result<OcrResult> {
        self.recognize_within(image, None)
    }

    /// Le so as linhas perto de `foco` (em pixels da imagem).
    ///
    /// # Por que existe
    ///
    /// A deteccao e barata e proporcional ao tamanho da imagem; o
    /// reconhecimento e caro e proporcional ao **numero de linhas**. Medido no
    /// uso real em 2026-08-01: uma tela cheia de texto dava ~250 palavras em
    /// ~1,2 s de reconhecimento — para responder sobre **uma** palavra, a que
    /// esta sob o cursor.
    ///
    /// Espiar nao precisa da tela: precisa da linha apontada e das vizinhas,
    /// que sao o resto da frase (ver [`sentences`]). Ler so essa vizinhanca
    /// tira o custo da densidade da tela e o prende ao que o usuario aponta.
    /// `linhas` limita quantas linhas em volta do foco sao reconhecidas.
    /// Ver [`Foco`] para as duas escolhas que o produto faz.
    pub fn recognize_near(
        &mut self,
        image: &RgbImage,
        foco: (u32, u32),
        linhas: usize,
    ) -> Result<OcrResult> {
        self.recognize_within(image, Some((foco, linhas)))
    }

    fn recognize_within(
        &mut self,
        image: &RgbImage,
        foco: Option<((u32, u32), usize)>,
    ) -> Result<OcrResult> {
        let caixas = self.detect(image)?;
        let mut agrupadas = lines::group(caixas, &self.params);
        if let Some((ponto, linhas)) = foco {
            agrupadas = perto_do_foco(agrupadas, ponto, linhas);
        }

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

/// Quanto contexto o reconhecimento deve ler em volta do cursor.
///
/// Existem duas respostas porque existem dois momentos, e cobrar o preco do
/// segundo no primeiro era o que deixava a espiada lenta (medido em
/// 2026-08-01: 47 palavras, 421 ms, para mostrar **uma** traducao).
pub struct Foco;

impl Foco {
    /// Espiando: so a linha sob o cursor. E tudo que o tooltip mostra, e o
    /// gesto tem que parecer colado no mouse.
    pub const TOOLTIP: usize = 1;

    /// Card aberto: a linha e ate tres de cada lado, que e a fala inteira
    /// (ver [`sentences`]). Custa mais, mas so depois de o usuario clicar —
    /// e ai ele ja esta esperando o card, que tambem traduz.
    pub const CARD: usize = 7;
}

/// Raio vertical do foco, em multiplos da altura da linha apontada.
///
/// Em alturas de linha, e nao em pixels, porque o que importa e "quantas
/// linhas de distancia" — e isso vale igual num menu de fonte pequena e numa
/// legenda grande.
const RAIO_EM_LINHAS: f32 = 3.5;

/// Fica com os grupos proximos do foco, em ordem de leitura.
///
/// Ordena por distancia para escolher, mas devolve na ordem original: o
/// agrupamento em frases ([`sentences`]) le linhas vizinhas em sequencia, e
/// baralhar aqui quebraria a frase.
fn perto_do_foco(grupos: Vec<Vec<BBox>>, (fx, fy): (u32, u32), maximo: usize) -> Vec<Vec<BBox>> {
    let envelopes: Vec<BBox> = grupos.iter().map(|g| lines::envelope(g)).collect();

    // O raio sai da altura da linha apontada (ou da mediana, se o cursor nao
    // estiver sobre nenhuma): usar a altura de uma linha qualquer daria raios
    // absurdos numa tela que mistura titulo e corpo.
    let altura = envelopes
        .iter()
        .find(|e| {
            fy >= e.y && fy < e.bottom() && fx >= e.x.saturating_sub(e.h) && fx <= e.right() + e.h
        })
        .map(|e| e.h)
        .or_else(|| {
            let mut alturas: Vec<u32> = envelopes.iter().map(|e| e.h).collect();
            alturas.sort_unstable();
            alturas.get(alturas.len() / 2).copied()
        })
        .unwrap_or(0);
    if altura == 0 {
        return grupos;
    }
    let raio = altura as f32 * RAIO_EM_LINHAS;

    let mut candidatos: Vec<(usize, f32)> = envelopes
        .iter()
        .enumerate()
        .map(|(i, e)| (i, (e.center_y() - fy as f32).abs()))
        .filter(|(_, distancia)| *distancia <= raio)
        .collect();
    candidatos.sort_by(|a, b| a.1.total_cmp(&b.1));
    candidatos.truncate(maximo.max(1));

    let mut escolhidos: Vec<usize> = candidatos.into_iter().map(|(i, _)| i).collect();
    escolhidos.sort_unstable();

    grupos
        .into_iter()
        .enumerate()
        .filter(|(i, _)| escolhidos.binary_search(i).is_ok())
        .map(|(_, grupo)| grupo)
        .collect()
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
///
/// `foco` em pixels do frame limita o reconhecimento a vizinhanca daquele
/// ponto — e o caminho da espiada. `None` le o frame inteiro, que e o que as
/// ferramentas de medicao e as fixtures querem.
pub fn recognize(
    engine: &mut Engine,
    frame: &Frame,
    foco: Option<((u32, u32), usize)>,
) -> Result<OcrResult> {
    let imagem = frame_para_rgb(frame)?;
    match foco {
        Some((ponto, linhas)) => engine.recognize_near(&imagem, ponto, linhas),
        None => engine.recognize_image(&imagem),
    }
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

    /// Vinte linhas empilhadas de 20px, como uma tela cheia de texto.
    fn tela_densa() -> Vec<Vec<BBox>> {
        (0..20)
            .map(|i| vec![bbox(100, 100 + i * 26, 400, 20)])
            .collect()
    }

    fn topos(grupos: &[Vec<BBox>]) -> Vec<u32> {
        grupos.iter().map(|g| lines::envelope(g).y).collect()
    }

    #[test]
    fn so_a_vizinhanca_do_cursor_vai_para_o_reconhecimento() {
        // O custo do reconhecimento e por linha: numa tela densa, ler tudo
        // custava >1 s para responder sobre uma palavra (medido em 2026-08-01).
        let escolhidos = perto_do_foco(tela_densa(), (300, 360), Foco::CARD);
        assert!(
            escolhidos.len() <= Foco::CARD,
            "{} linhas passaram",
            escolhidos.len()
        );
        assert!(!escolhidos.is_empty(), "a linha apontada tem que passar");
    }

    #[test]
    fn o_tooltip_le_uma_linha_so() {
        // A diferenca entre os dois momentos: espiando, uma traducao basta, e
        // o gesto tem que acompanhar o mouse. Sete linhas custavam 421 ms.
        let escolhidos = perto_do_foco(tela_densa(), (300, 370), Foco::TOOLTIP);
        assert_eq!(escolhidos.len(), 1);
        assert_eq!(topos(&escolhidos), vec![360], "e a linha apontada");
    }

    #[test]
    fn a_linha_apontada_esta_entre_as_escolhidas() {
        // Cursor no meio da linha que comeca em y=360.
        let escolhidos = perto_do_foco(tela_densa(), (300, 370), Foco::CARD);
        assert!(
            topos(&escolhidos).contains(&360),
            "{:?}",
            topos(&escolhidos)
        );
    }

    #[test]
    fn as_vizinhas_passam_junto_para_a_frase_nao_truncar() {
        // A fala de jogo atravessa linhas: sem as vizinhas, o card e a traducao
        // sairiam pela metade (ver `ocr::sentences`).
        let escolhidos = topos(&perto_do_foco(tela_densa(), (300, 370), Foco::CARD));
        assert!(escolhidos.contains(&334), "a de cima: {escolhidos:?}");
        assert!(escolhidos.contains(&386), "a de baixo: {escolhidos:?}");
    }

    #[test]
    fn as_escolhidas_saem_em_ordem_de_leitura() {
        // A busca da frase le linhas em sequencia; fora de ordem, ela quebra.
        let topos = topos(&perto_do_foco(tela_densa(), (300, 370), Foco::CARD));
        let mut ordenados = topos.clone();
        ordenados.sort_unstable();
        assert_eq!(topos, ordenados);
    }

    #[test]
    fn texto_longe_do_cursor_fica_de_fora() {
        let escolhidos = topos(&perto_do_foco(tela_densa(), (300, 110), Foco::CARD));
        assert!(
            !escolhidos.iter().any(|&y| y > 300),
            "linha distante entrou: {escolhidos:?}"
        );
    }

    #[test]
    fn tela_sem_texto_nao_entra_em_panico() {
        assert!(perto_do_foco(Vec::new(), (300, 300), Foco::CARD).is_empty());
    }

    #[test]
    fn limite_zero_ainda_devolve_a_linha_apontada() {
        // Defesa contra um `Foco` mal configurado: zero linha seria um tooltip
        // que nunca acha palavra nenhuma.
        assert_eq!(perto_do_foco(tela_densa(), (300, 370), 0).len(), 1);
    }

    #[test]
    fn o_raio_acompanha_o_tamanho_da_fonte() {
        // Mesma distancia em pixels, alturas diferentes: numa fonte grande as
        // linhas vizinhas ainda sao vizinhas; numa pequena, ja sao outro bloco.
        let grande: Vec<Vec<BBox>> = (0..6)
            .map(|i| vec![bbox(100, 100 + i * 60, 400, 48)])
            .collect();
        let pequena: Vec<Vec<BBox>> = (0..6)
            .map(|i| vec![bbox(100, 100 + i * 60, 400, 10)])
            .collect();
        let com_fonte_grande = perto_do_foco(grande, (300, 130), Foco::CARD).len();
        let com_fonte_pequena = perto_do_foco(pequena, (300, 105), Foco::CARD).len();
        assert!(
            com_fonte_grande > com_fonte_pequena,
            "grande {com_fonte_grande}, pequena {com_fonte_pequena}"
        );
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
