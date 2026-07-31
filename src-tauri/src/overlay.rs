//! Janela overlay: alterna entre modo passivo (click-through) e modo lookup
//! (interativo), cobrindo o monitor da janela em foco.
//!
//! Spike 01 — ver `docs/spikes/spike-01-overlay.md`.
//!
//! Invariantes:
//! - A overlay fica **sempre visivel**; o que muda e so quem recebe o clique.
//!   Isso mantem o custo de entrar em lookup baixo (sem show/hide da janela) e
//!   permite desenhar destaques passivos no futuro.
//! - Ao sair do modo lookup o foco volta para a janela que o tinha antes,
//!   senao o jogo continua sem receber teclado.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize};

use crate::error::{Error, Result};
use crate::platform;

/// Label da janela overlay em `tauri.conf.json`.
pub const OVERLAY_LABEL: &str = "overlay";

/// `true` = modo lookup (recebe cliques); `false` = passivo (click-through).
static INTERACTIVE: AtomicBool = AtomicBool::new(false);

/// Janela que tinha o foco quando entramos em lookup (0 = nenhuma).
static PREVIOUS_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

/// Alvo congelado no instante em que entramos em lookup.
///
/// Em modo lookup **a overlay e a janela em foco**, entao consultar o Windows
/// durante a consulta devolveria "PapaPlay Overlay" no lugar do nome do jogo.
/// O alvo certo e o de antes da troca de modo, e e este.
static LOOKUP_TARGET: Mutex<Option<platform::ForegroundTarget>> = Mutex::new(None);

/// Jogo e monitor de onde a consulta atual deve ler. `None` fora do lookup.
pub fn lookup_target() -> Option<platform::ForegroundTarget> {
    LOOKUP_TARGET.lock().ok().and_then(|alvo| alvo.clone())
}

/// Resultado de uma troca de modo — vai para a UI pelo evento `overlay://mode`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeChange {
    pub interactive: bool,
    /// Duracao da troca de modo em microssegundos (medida no core).
    pub elapsed_us: u128,
    /// Titulo da janela que estava em foco ao entrar em lookup.
    pub window_title: Option<String>,
    /// Monitor coberto pela overlay, em pixels fisicos.
    pub monitor: Option<platform::MonitorRect>,
    /// Escala de DPI do monitor onde a overlay ficou.
    pub scale_factor: f64,
}

/// Estado atual, para a UI se sincronizar ao montar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayStatus {
    pub interactive: bool,
    pub scale_factor: f64,
}

pub fn is_interactive() -> bool {
    INTERACTIVE.load(Ordering::SeqCst)
}

/// Move e redimensiona a overlay para cobrir exatamente o monitor alvo.
///
/// Usa pixels **fisicos**: `MONITORINFO` ja vem em coordenadas fisicas da area
/// de trabalho virtual, entao nao ha conversao de DPI no meio do caminho — o
/// que evita o classico erro de meio-monitor em setups com escalas diferentes.
fn cover_monitor(window: &tauri::WebviewWindow, monitor: platform::MonitorRect) -> Result<()> {
    window.set_position(PhysicalPosition::new(monitor.x, monitor.y))?;
    window.set_size(PhysicalSize::new(
        monitor.width.max(1) as u32,
        monitor.height.max(1) as u32,
    ))?;
    Ok(())
}

/// HWND da propria overlay, para nao "restaurar" o foco para nos mesmos.
#[cfg(windows)]
fn own_hwnd(window: &tauri::WebviewWindow) -> isize {
    window.hwnd().map(|h| h.0 as isize).unwrap_or(0)
}

#[cfg(not(windows))]
fn own_hwnd(_window: &tauri::WebviewWindow) -> isize {
    0
}

/// Aplica um modo. Idempotente: chamar duas vezes com o mesmo valor so repete
/// o posicionamento, o que e util quando o jogo troca de monitor.
pub fn set_mode(app: &AppHandle, interactive: bool) -> Result<ModeChange> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(Error::WindowNotFound(OVERLAY_LABEL))?;

    let started = Instant::now();
    let mut window_title = None;
    let mut monitor = None;

    if interactive {
        // Le o alvo ANTES de roubar o foco — depois disso a janela em primeiro
        // plano passa a ser a propria overlay.
        let target = platform::foreground_target()?;
        if target.hwnd != own_hwnd(&window) {
            PREVIOUS_FOREGROUND.store(target.hwnd, Ordering::SeqCst);
            // Alt+X duas vezes seguidas nao pode substituir o jogo pela propria
            // overlay como alvo da consulta.
            if let Ok(mut alvo) = LOOKUP_TARGET.lock() {
                *alvo = Some(target.clone());
            }
        }
        cover_monitor(&window, target.monitor)?;
        window.set_ignore_cursor_events(false)?;
        window.set_always_on_top(true)?;
        window.show()?;
        window.set_focus()?;
        window_title = target.window_title;
        monitor = Some(target.monitor);
    } else {
        window.set_ignore_cursor_events(true)?;
        // A overlay continua visivel — so para de interceptar o mouse.
        let previous = PREVIOUS_FOREGROUND.swap(0, Ordering::SeqCst);
        platform::restore_foreground(previous);
        if let Ok(mut alvo) = LOOKUP_TARGET.lock() {
            *alvo = None;
        }
    }

    // Parte da transicao, entao entra na medicao: o Esc global so pode existir
    // enquanto estamos em lookup.
    crate::hotkeys::sync_escape(app, interactive)?;
    INTERACTIVE.store(interactive, Ordering::SeqCst);

    let change = ModeChange {
        interactive,
        elapsed_us: started.elapsed().as_micros(),
        window_title,
        monitor,
        scale_factor: window.scale_factor().unwrap_or(1.0),
    };
    app.emit("overlay://mode", change.clone())?;
    Ok(change)
}

/// Inverte o modo atual. E o que a hotkey `Alt+X` chama.
pub fn toggle(app: &AppHandle) -> Result<ModeChange> {
    set_mode(app, !is_interactive())
}

/// Deixa a overlay pronta no boot: cobrindo o monitor em foco, visivel e
/// passiva. O modo passivo e o estado de repouso do app.
pub fn init(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(Error::WindowNotFound(OVERLAY_LABEL))?;

    if let Ok(target) = platform::foreground_target() {
        cover_monitor(&window, target.monitor)?;
    }
    // Tira a overlay da propria captura. Sem isto os destaques de uma consulta
    // apareceriam como texto na consulta seguinte — o WGC fotografa o monitor
    // ja composto, com as nossas janelas dentro.
    platform::exclude_from_capture(own_hwnd(&window));
    window.set_ignore_cursor_events(true)?;
    window.set_always_on_top(true)?;
    window.show()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Verificacao de geometria
// ---------------------------------------------------------------------------

/// Compara o retangulo do monitor alvo com o que a janela realmente ocupou.
///
/// E o teste que pega o erro classico de DPI: pedir tamanho em pixels logicos
/// num monitor com escala != 100% cobre so um pedaco da tela.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryCheck {
    pub monitor: platform::MonitorRect,
    pub window: platform::MonitorRect,
    pub scale_factor: f64,
    /// `true` se a janela cobre o monitor exatamente (tolerancia de 1px).
    pub matches: bool,
}

/// Entra em modo lookup, mede a geometria resultante e volta para passivo.
pub fn check_geometry(app: &AppHandle) -> Result<GeometryCheck> {
    let change = set_mode(app, true)?;
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(Error::WindowNotFound(OVERLAY_LABEL))?;

    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let actual = platform::MonitorRect {
        x: position.x,
        y: position.y,
        width: size.width as i32,
        height: size.height as i32,
    };
    let monitor = change
        .monitor
        .ok_or_else(|| Error::Platform("monitor alvo nao reportado".into()))?;

    let matches = (actual.x - monitor.x).abs() <= 1
        && (actual.y - monitor.y).abs() <= 1
        && (actual.width - monitor.width).abs() <= 1
        && (actual.height - monitor.height).abs() <= 1;

    set_mode(app, false)?;
    Ok(GeometryCheck {
        monitor,
        window: actual,
        scale_factor: change.scale_factor,
        matches,
    })
}

// ---------------------------------------------------------------------------
// Benchmark da spike
// ---------------------------------------------------------------------------

/// Estatisticas de N alternancias seguidas (criterio GO: p100 < 150ms).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchReport {
    pub iterations: usize,
    pub min_us: u128,
    pub max_us: u128,
    pub mean_us: u128,
    pub p50_us: u128,
    pub p95_us: u128,
    /// Quantas alternancias falharam (qualquer erro do core).
    pub failures: usize,
    /// Amostras individuais, na ordem em que foram medidas.
    pub samples_us: Vec<u128>,
}

fn summarize(mut samples: Vec<u128>, failures: usize) -> BenchReport {
    let iterations = samples.len() + failures;
    if samples.is_empty() {
        return BenchReport {
            iterations,
            min_us: 0,
            max_us: 0,
            mean_us: 0,
            p50_us: 0,
            p95_us: 0,
            failures,
            samples_us: samples,
        };
    }
    let sum: u128 = samples.iter().sum();
    let mean = sum / samples.len() as u128;
    let ordered = {
        let mut o = samples.clone();
        o.sort_unstable();
        o
    };
    let pick = |q: f64| {
        let idx = ((ordered.len() as f64 - 1.0) * q).round() as usize;
        ordered[idx]
    };
    samples.shrink_to_fit();
    BenchReport {
        iterations,
        min_us: ordered[0],
        max_us: ordered[ordered.len() - 1],
        mean_us: mean,
        p50_us: pick(0.50),
        p95_us: pick(0.95),
        failures,
        samples_us: samples,
    }
}

/// Executa `iterations` alternancias (cada uma = uma troca de modo) e devolve
/// as estatisticas. Roda numa thread propria porque as chamadas de janela sao
/// dispatches sincronos para o event loop.
fn run_bench(app: &AppHandle, iterations: usize) -> BenchReport {
    let mut samples = Vec::with_capacity(iterations);
    let mut failures = 0usize;

    for i in 0..iterations {
        match set_mode(app, i % 2 == 0) {
            Ok(change) => samples.push(change.elapsed_us),
            Err(_) => failures += 1,
        }
        // Deixa o compositor assentar entre as trocas; fora da medicao.
        std::thread::sleep(Duration::from_millis(10));
    }

    // Termina sempre em modo passivo.
    let _ = set_mode(app, false);
    summarize(samples, failures)
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------
//
// Todos `async` de proposito: comandos sincronos do Tauri rodam na main thread
// e as chamadas de janela abaixo fazem dispatch bloqueante para ela.

#[tauri::command]
pub async fn overlay_set_mode(app: AppHandle, interactive: bool) -> Result<ModeChange> {
    set_mode(&app, interactive)
}

#[tauri::command]
pub async fn overlay_toggle(app: AppHandle) -> Result<ModeChange> {
    toggle(&app)
}

#[tauri::command]
pub async fn overlay_status(app: AppHandle) -> Result<OverlayStatus> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(Error::WindowNotFound(OVERLAY_LABEL))?;
    Ok(OverlayStatus {
        interactive: is_interactive(),
        scale_factor: window.scale_factor().unwrap_or(1.0),
    })
}

#[tauri::command]
pub async fn overlay_check_geometry(app: AppHandle) -> Result<GeometryCheck> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || check_geometry(&handle))
        .await
        .map_err(|e| Error::Platform(format!("checagem de geometria abortada: {e}")))?
}

#[tauri::command]
pub async fn overlay_bench(app: AppHandle, iterations: usize) -> Result<BenchReport> {
    let iterations = iterations.clamp(1, 500);
    let handle = app.clone();
    let report = tauri::async_runtime::spawn_blocking(move || run_bench(&handle, iterations))
        .await
        .map_err(|e| Error::Platform(format!("benchmark abortado: {e}")))?;
    app.emit("overlay://bench", report.clone())?;
    Ok(report)
}

/// Só para conferir que a janela principal continua utilizavel durante a spike.
#[tauri::command]
pub async fn overlay_reset_size(app: AppHandle) -> Result<()> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(Error::WindowNotFound(OVERLAY_LABEL))?;
    window.set_size(LogicalSize::new(1280.0, 720.0))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resumo_calcula_percentis_sobre_as_amostras_ordenadas() {
        // Fora de ordem de proposito: o p50/p95 nao pode depender da ordem
        // em que as alternancias aconteceram.
        let report = summarize(vec![50, 10, 40, 20, 30], 0);
        assert_eq!(report.iterations, 5);
        assert_eq!(report.min_us, 10);
        assert_eq!(report.max_us, 50);
        assert_eq!(report.mean_us, 30);
        assert_eq!(report.p50_us, 30);
        assert_eq!(report.p95_us, 50);
        // As amostras voltam na ordem original, para o relatorio da spike.
        assert_eq!(report.samples_us, vec![50, 10, 40, 20, 30]);
    }

    #[test]
    fn resumo_conta_falhas_no_total_de_iteracoes() {
        let report = summarize(vec![10, 20], 3);
        assert_eq!(report.iterations, 5);
        assert_eq!(report.failures, 3);
    }

    #[test]
    fn resumo_de_zero_amostras_nao_entra_em_panico() {
        let report = summarize(Vec::new(), 4);
        assert_eq!(report.iterations, 4);
        assert_eq!(report.max_us, 0);
    }
}
