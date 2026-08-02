//! Consulta: captura -> OCR -> palavras posicionadas na overlay.
//!
//! E a costura da F2. As tres pecas ja existiam separadas; o que este modulo
//! resolve e o que fica entre elas:
//!
//! - **Qual alvo ler.** Em modo lookup a janela em foco e a propria overlay,
//!   entao o alvo vem congelado de [`overlay::lookup_target`].
//! - **Onde ler.** So o entorno do cursor ([`capture::LOOKUP_REGION`]) — a
//!   spike 02 mediu que ler a tela inteira estoura o orcamento de 1 s.
//! - **Em que coordenadas devolver.** O OCR fala em pixels fisicos relativos ao
//!   recorte; a overlay desenha em pixels logicos relativos a si mesma. A
//!   conversao mora em [`para_overlay`] e em lugar nenhum mais, que e como se
//!   evita o classico erro de DPI de posicionar destaque em cima da palavra
//!   errada.
//!
//! O dicionario e a traducao entram depois, consumindo `LookupResult`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::capture::{self, Frame, Region, LOOKUP_REGION};
use crate::error::{Error, Result};
use crate::media;
use crate::ocr::{self, BBox, Engine};
use crate::overlay;
use crate::platform::{self, MonitorRect};

/// Variavel de ambiente que sobrescreve o diretorio dos modelos de OCR.
const MODELS_ENV: &str = "PAPAPLAY_OCR_MODELS";

/// Carregar os modelos custa ~150 ms e ~35 MB. Uma vez carregados, ficam: em
/// troca de RAM, toda consulta depois da primeira paga so a inferencia.
/// (A traducao NMT tera a politica oposta — ver `translate::unload_model`.)
static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

/// Frame da consulta atual, guardado inteiro para poder recortar o screenshot
/// do card sem capturar a tela de novo — no instante do clique em "salvar" o
/// jogo ja avancou e a frase pode nem estar mais la.
///
/// Custa ~3,5 MB (1280x720 BGRA) e vive so enquanto o modo lookup esta ligado:
/// [`forget_capture`] o solta ao voltar para o passivo, junto com o tradutor.
static ULTIMA_CAPTURA: Mutex<Option<Captura>> = Mutex::new(None);

/// Identifica a consulta. Serve para o salvamento provar que o recorte que ele
/// quer e da tela que o usuario esta vendo, e nao de uma consulta posterior.
static PROXIMO_ID: AtomicU64 = AtomicU64::new(1);

struct Captura {
    id: u64,
    /// O frame como veio da tela. Guardado inteiro porque serve a dois donos:
    /// o recorte do screenshot e a releitura ampliada do card.
    frame: Frame,
    /// Caixa de cada linha reconhecida, em pixels **do frame** — nao as da
    /// overlay: recorte se faz na imagem, nao na tela.
    lines: Vec<BBox>,
}

/// Retangulo em pixels **logicos**, relativo ao canto da overlay — pronto para
/// virar `style="left: {x}px; top: {y}px"` sem mais nenhuma conversao.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl OverlayRect {
    /// `true` se o ponto (em pixels logicos da overlay) cai dentro.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupWord {
    pub text: String,
    pub rect: OverlayRect,
    pub conf: f32,
    /// Indice da linha em [`LookupResult::lines`] — e dela que sai a frase de
    /// contexto do card.
    pub line_index: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupLine {
    pub text: String,
    pub rect: OverlayRect,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupResult {
    /// Identificador desta consulta. Volta em `deck_save_card` para dizer de
    /// qual captura sai o screenshot do contexto.
    pub lookup_id: u64,
    pub words: Vec<LookupWord>,
    pub lines: Vec<LookupLine>,
    /// Cursor em pixels logicos da overlay, para saber qual palavra esta sob
    /// ele. `None` quando nao ha cursor (sessao remota).
    pub cursor: Option<(f64, f64)>,
    /// Nome do jogo, para o card.
    pub game_name: Option<String>,
    /// Recorte lido, em pixels fisicos — util para diagnostico.
    pub region: Region,
    pub capture_ms: f64,
    pub ocr_ms: f64,
    pub total_ms: f64,
}

/// Converte uma bbox do recorte para o retangulo logico da overlay.
///
/// Tres sistemas de coordenadas em sequencia:
/// 1. `bbox` e relativa ao recorte capturado;
/// 2. `origem_frame` situa o recorte na area de trabalho virtual (fisico);
/// 3. a overlay cobre `monitor`, e desenha em pixels logicos (fisico / escala).
fn para_overlay(
    bbox: BBox,
    origem_frame: (i32, i32),
    monitor: MonitorRect,
    escala: f64,
) -> OverlayRect {
    // Escala zero ou negativa so aparece se a API de DPI devolver lixo; tratar
    // aqui evita divisao por zero virar NaN no meio do CSS.
    let escala = if escala > 0.0 { escala } else { 1.0 };
    let x = (origem_frame.0 + bbox.x as i32 - monitor.x) as f64 / escala;
    let y = (origem_frame.1 + bbox.y as i32 - monitor.y) as f64 / escala;
    OverlayRect {
        x,
        y,
        w: bbox.w as f64 / escala,
        h: bbox.h as f64 / escala,
    }
}

/// Diretorio dos modelos ONNX, na ordem: variavel de ambiente, recursos do
/// app empacotado, arvore do repo (desenvolvimento).
fn models_dir(app: &AppHandle) -> Result<PathBuf> {
    if let Some(bruto) = std::env::var_os(MODELS_ENV) {
        return Ok(PathBuf::from(bruto));
    }
    if let Ok(recursos) = app.path().resource_dir() {
        let candidato = recursos.join("models");
        if candidato.join("en_dict.txt").is_file() {
            return Ok(candidato);
        }
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/models");
    if repo.join("en_dict.txt").is_file() {
        return Ok(repo);
    }
    Err(Error::Ocr(format!(
        "modelos de OCR nao encontrados — rode `powershell -File scripts/fetch-ocr-models.ps1` \
         ou aponte {MODELS_ENV} para o diretorio deles"
    )))
}

/// Roda uma consulta em volta do cursor e devolve as palavras posicionadas.
pub fn run(app: &AppHandle) -> Result<LookupResult> {
    // Fora de uma espiada (probe, teste manual) nao ha alvo congelado, e ai a
    // janela em foco e mesmo o alvo certo.
    let target = match overlay::lookup_target() {
        Some(alvo) => alvo,
        None => platform::foreground_target()?,
    };
    read_around(app, &target, None, LOOKUP_REGION, ocr::Foco::CARD)
}

/// Le uma regiao de `size` em volta de `center` (ou do cursor, se `None`).
///
/// E o passo que a espiada repete enquanto o cursor anda: alvo e tamanho vem de
/// fora justamente porque quem espia le varias vezes, em pontos diferentes, sem
/// nunca reconsultar qual e a janela do jogo — ela ja foi congelada quando a
/// tecla desceu.
pub fn read_around(
    app: &AppHandle,
    target: &platform::ForegroundTarget,
    center: Option<(i32, i32)>,
    size: (u32, u32),
    // `linhas`: quantas linhas em volta do cursor reconhecer (ver `ocr::Foco`).
    linhas: usize,
) -> Result<LookupResult> {
    let comeco = Instant::now();

    let t_captura = Instant::now();
    let frame = capture::capture_region_of(target, size, center)?;
    let capture_ms = ms(t_captura);

    // Onde o cursor caiu dentro do recorte. E o que diz ao OCR quais linhas
    // valem o reconhecimento: ler a tela toda para responder sobre uma palavra
    // custava mais de 1 s em telas densas (medido em 2026-08-01).
    let foco = center.or_else(platform::cursor_pos).map(|(x, y)| {
        (
            (
                (x - frame.x).clamp(0, frame.width.saturating_sub(1) as i32) as u32,
                (y - frame.y).clamp(0, frame.height.saturating_sub(1) as i32) as u32,
            ),
            linhas,
        )
    });

    let t_ocr = Instant::now();
    let resultado = {
        let mut guarda = ENGINE
            .lock()
            .map_err(|_| Error::Ocr("engine de OCR envenenada".into()))?;
        if guarda.is_none() {
            *guarda = Some(Engine::load(&models_dir(app)?)?);
        }
        let engine = guarda.as_mut().expect("engine acabou de ser carregada");
        ocr::recognize(engine, &frame, foco)?
    };
    let ocr_ms = ms(t_ocr);

    let mut saida = montar(resultado, frame, target);
    saida.capture_ms = capture_ms;
    saida.ocr_ms = ocr_ms;
    saida.total_ms = ms(comeco);
    Ok(saida)
}

/// Do resultado do OCR (pixels do recorte) para o que a overlay desenha
/// (pixels logicos dela), guardando o frame no caminho.
///
/// As duas leituras da espiada terminam aqui — a que captura e a que reprocessa
/// o frame guardado ([`expand_stored`]) —, e e o que garante que as duas
/// devolvam coordenadas no mesmo sistema.
fn montar(
    resultado: ocr::OcrResult,
    frame: Frame,
    target: &platform::ForegroundTarget,
) -> LookupResult {
    let origem = (frame.x, frame.y);
    let escala = target.scale_factor;
    let monitor = target.monitor;
    let region = Region {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
    };

    // Antes de mapear para a overlay: o recorte do screenshot precisa das
    // caixas no sistema de coordenadas da imagem.
    let lookup_id = guardar_captura(
        frame,
        resultado.lines.iter().map(|linha| linha.bbox).collect(),
    );

    let words = resultado
        .words
        .into_iter()
        .map(|palavra| LookupWord {
            text: palavra.text,
            rect: para_overlay(palavra.bbox, origem, monitor, escala),
            conf: palavra.conf,
            line_index: palavra.line_index,
        })
        .collect();

    let lines = resultado
        .lines
        .into_iter()
        .map(|linha| LookupLine {
            text: linha.text,
            rect: para_overlay(linha.bbox, origem, monitor, escala),
        })
        .collect();

    let cursor = platform::cursor_pos().map(|(x, y)| {
        (
            (x - monitor.x) as f64 / escala.max(f64::EPSILON),
            (y - monitor.y) as f64 / escala.max(f64::EPSILON),
        )
    });

    LookupResult {
        lookup_id,
        words,
        lines,
        cursor,
        game_name: target.window_title.clone(),
        region,
        capture_ms: 0.0,
        ocr_ms: 0.0,
        total_ms: 0.0,
    }
}

fn ms(desde: Instant) -> f64 {
    desde.elapsed().as_secs_f64() * 1000.0
}

/// Guarda o frame da consulta e devolve o id dela.
///
/// Substitui a captura anterior: so a tela que o usuario esta vendo interessa,
/// e manter as antigas seria acumular megabytes por Alt+X.
fn guardar_captura(frame: Frame, lines: Vec<BBox>) -> u64 {
    let id = PROXIMO_ID.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut guarda) = ULTIMA_CAPTURA.lock() {
        *guarda = Some(Captura { id, frame, lines });
    }
    id
}

/// Recorta a linha `line_index` da captura `lookup_id` — o screenshot que vai
/// para o contexto do card.
///
/// `None` quando a captura ja foi descartada ou quando outra consulta rodou no
/// meio: card sem imagem e um card incompleto, mas card com a imagem de outra
/// tela e um card errado.
pub fn recortar_linha(lookup_id: u64, line_index: usize) -> Option<media::Recorte> {
    let guarda = ULTIMA_CAPTURA.lock().ok()?;
    let captura = guarda.as_ref()?;
    if captura.id != lookup_id {
        return None;
    }
    let area = *captura.lines.get(line_index)?;
    media::recortar(
        &media::Bgra {
            width: captura.frame.width,
            height: captura.frame.height,
            pixels: &captura.frame.pixels,
        },
        area,
        media::MARGEM,
    )
}

/// Le de novo o frame ja capturado, agora com a vizinhanca inteira da linha.
///
/// E o segundo tempo da espiada. O primeiro le uma linha so, porque o tooltip
/// mostra uma traducao e precisa acompanhar o mouse; ao clicar, o card quer a
/// **frase**, que costuma atravessar linhas — e ai vale pagar o reconhecimento
/// das vizinhas.
///
/// Reaproveita o frame em memoria em vez de capturar de novo: entre o hover e
/// o clique o jogo avancou, e uma captura nova traria outra tela. Isso tambem
/// tira a captura (~20 ms) do caminho.
pub fn expand_stored(
    app: &AppHandle,
    lookup_id: u64,
    target: &platform::ForegroundTarget,
) -> Result<Option<LookupResult>> {
    let (frame, cursor) = {
        let guarda = ULTIMA_CAPTURA
            .lock()
            .map_err(|_| Error::Ocr("captura envenenada".into()))?;
        let Some(captura) = guarda.as_ref().filter(|c| c.id == lookup_id) else {
            // Espiada encerrada ou leitura mais nova no lugar: quem chamou
            // segue com o que ja tinha.
            return Ok(None);
        };
        // Clonar os pixels (~2 MB) para soltar o cadeado antes do OCR: segurar
        // durante a inferencia travaria a leitura seguinte.
        (clonar(&captura.frame), platform::cursor_pos())
    };

    let foco = cursor.map(|(x, y)| {
        (
            (
                (x - frame.x).clamp(0, frame.width.saturating_sub(1) as i32) as u32,
                (y - frame.y).clamp(0, frame.height.saturating_sub(1) as i32) as u32,
            ),
            ocr::Foco::CARD,
        )
    });

    let resultado = {
        let mut guarda = ENGINE
            .lock()
            .map_err(|_| Error::Ocr("engine de OCR envenenada".into()))?;
        if guarda.is_none() {
            *guarda = Some(Engine::load(&models_dir(app)?)?);
        }
        ocr::recognize(
            guarda.as_mut().expect("engine carregada"),
            &frame,
            foco,
        )?
    };

    Ok(Some(montar(resultado, frame, target)))
}

fn clonar(frame: &Frame) -> Frame {
    Frame {
        width: frame.width,
        height: frame.height,
        x: frame.x,
        y: frame.y,
        scale_factor: frame.scale_factor,
        window_title: frame.window_title.clone(),
        pixels: frame.pixels.clone(),
    }
}

/// Solta o frame da ultima consulta. Chamado ao sair do modo lookup.
pub fn forget_capture() {
    if let Ok(mut guarda) = ULTIMA_CAPTURA.lock() {
        *guarda = None;
    }
}

/// Carrega os modelos de OCR numa thread de fundo, para a primeira espiada
/// nao pagar os ~170 ms de carga.
///
/// Silencioso de proposito: se os modelos nao estiverem no lugar, quem avisa e
/// a primeira espiada, com a mensagem completa de [`models_dir`]. Reclamar no
/// boot seria um erro sobre algo que o usuario ainda nem tentou usar.
pub fn preload_engine(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(dir) = models_dir(&app) else { return };
        let Ok(engine) = Engine::load(&dir) else { return };
        if let Ok(mut guarda) = ENGINE.lock() {
            // So preenche se ninguem chegou antes: uma espiada disparada
            // durante o boot ja carregou o que precisava.
            if guarda.is_none() {
                *guarda = Some(engine);
            }
        }
    });
}

/// Descarrega os modelos de OCR. Existe para a tela de configuracoes poder
/// devolver a RAM sem fechar o app.
pub fn unload_engine() {
    if let Ok(mut guarda) = ENGINE.lock() {
        *guarda = None;
    }
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

/// `async` de proposito: a consulta bloqueia por centenas de milissegundos e
/// nao pode segurar a main thread, que e quem desenha a overlay.
#[tauri::command]
pub async fn lookup_run(app: AppHandle) -> Result<LookupResult> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || run(&handle))
        .await
        .map_err(|e| Error::Ocr(format!("consulta abortada: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> MonitorRect {
        MonitorRect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        }
    }

    fn bbox(x: u32, y: u32, w: u32, h: u32) -> BBox {
        BBox { x, y, w, h }
    }

    #[test]
    fn bbox_do_recorte_soma_a_origem_do_recorte() {
        // Palavra a 100,50 dentro de um recorte que comeca em 640,360.
        let r = para_overlay(bbox(100, 50, 80, 20), (640, 360), monitor(), 1.0);
        assert_eq!(
            r,
            OverlayRect {
                x: 740.0,
                y: 410.0,
                w: 80.0,
                h: 20.0
            }
        );
    }

    #[test]
    fn escala_de_dpi_divide_posicao_e_tamanho() {
        // Monitor a 150%: a overlay desenha em pixels logicos, entao tudo
        // encolhe por 1.5 — nao so o tamanho, a posicao tambem.
        let r = para_overlay(bbox(300, 150, 90, 30), (0, 0), monitor(), 1.5);
        assert_eq!(
            r,
            OverlayRect {
                x: 200.0,
                y: 100.0,
                w: 60.0,
                h: 20.0
            }
        );
    }

    #[test]
    fn monitor_com_origem_negativa_vira_coordenada_local_da_overlay() {
        // A overlay cobre o monitor, entao o canto dela e o canto do monitor:
        // uma palavra no canto do monitor secundario tem que sair em 0,0.
        let secundario = MonitorRect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let r = para_overlay(bbox(0, 0, 50, 10), (-1920, 0), secundario, 1.0);
        assert_eq!((r.x, r.y), (0.0, 0.0));
    }

    #[test]
    fn escala_invalida_nao_produz_nan() {
        let r = para_overlay(bbox(10, 10, 10, 10), (0, 0), monitor(), 0.0);
        assert!(r.x.is_finite() && r.w.is_finite(), "{r:?}");
        assert_eq!(r.w, 10.0, "escala invalida cai em 1.0");
    }

    /// Um so teste mexe na captura guardada: ela e um estatico do processo e
    /// dois testes paralelos brigariam por ela.
    #[test]
    fn o_recorte_sai_da_captura_da_propria_consulta() {
        let frame = Frame {
            width: 200,
            height: 100,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            window_title: None,
            pixels: vec![255u8; 200 * 100 * 4],
        };
        let id = guardar_captura(frame, vec![bbox(80, 40, 40, 12)]);

        let recorte = recortar_linha(id, 0).expect("recortou a linha 0");
        assert_eq!(recorte.width, 40 + media::MARGEM.0 * 2, "linha mais margem");
        assert_eq!(recorte.height, 12 + media::MARGEM.1 * 2);

        assert!(recortar_linha(id, 7).is_none(), "linha que nao existe");
        // O caso que o id existe para pegar: o usuario deu Alt+X de novo antes
        // de clicar em salvar, e o recorte viria da tela errada.
        assert!(recortar_linha(id + 1, 0).is_none(), "outra consulta");

        forget_capture();
        assert!(recortar_linha(id, 0).is_none(), "captura ja descartada");
    }

    #[test]
    fn retangulo_contem_o_proprio_canto_mas_nao_a_borda_oposta() {
        let r = OverlayRect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 30.0,
        };
        assert!(r.contains(10.0, 20.0), "canto superior esquerdo entra");
        assert!(r.contains(60.0, 35.0), "meio entra");
        assert!(!r.contains(110.0, 35.0), "borda direita fica de fora");
        assert!(!r.contains(60.0, 50.0), "borda inferior fica de fora");
        assert!(!r.contains(9.0, 35.0), "antes do canto fica de fora");
    }
}
