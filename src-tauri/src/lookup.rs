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
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::capture::{self, Region, LOOKUP_REGION};
use crate::error::{Error, Result};
use crate::ocr::{self, BBox, Engine};
use crate::overlay;
use crate::platform::{self, MonitorRect};

/// Variavel de ambiente que sobrescreve o diretorio dos modelos de OCR.
const MODELS_ENV: &str = "PAPAPLAY_OCR_MODELS";

/// Carregar os modelos custa ~150 ms e ~35 MB. Uma vez carregados, ficam: em
/// troca de RAM, toda consulta depois da primeira paga so a inferencia.
/// (A traducao NMT tera a politica oposta — ver `translate::unload_model`.)
static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

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
    let comeco = Instant::now();

    // Fora do modo lookup (probe, teste manual) nao ha alvo congelado, e ai a
    // janela em foco e mesmo o alvo certo.
    let target = match overlay::lookup_target() {
        Some(alvo) => alvo,
        None => platform::foreground_target()?,
    };

    let t_captura = Instant::now();
    let frame = capture::capture_region_of(&target, LOOKUP_REGION, None)?;
    let capture_ms = ms(t_captura);

    let t_ocr = Instant::now();
    let resultado = {
        let mut guarda = ENGINE
            .lock()
            .map_err(|_| Error::Ocr("engine de OCR envenenada".into()))?;
        if guarda.is_none() {
            *guarda = Some(Engine::load(&models_dir(app)?)?);
        }
        let engine = guarda.as_mut().expect("engine acabou de ser carregada");
        ocr::recognize(engine, &frame)?
    };
    let ocr_ms = ms(t_ocr);

    let origem = (frame.x, frame.y);
    let escala = target.scale_factor;
    let monitor = target.monitor;

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

    Ok(LookupResult {
        words,
        lines,
        cursor,
        game_name: target.window_title,
        region: Region {
            x: frame.x,
            y: frame.y,
            width: frame.width,
            height: frame.height,
        },
        capture_ms,
        ocr_ms,
        total_ms: ms(comeco),
    })
}

fn ms(desde: Instant) -> f64 {
    desde.elapsed().as_secs_f64() * 1000.0
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
