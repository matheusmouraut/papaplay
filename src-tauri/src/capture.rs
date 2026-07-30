//! Captura de tela via Windows Graphics Capture.
//!
//! REGRA INVIOLAVEL: captura externa apenas. Nunca injetar DLL nem instalar
//! hook no processo do jogo (risco de anti-cheat). Ver CLAUDE.md.
//!
//! Implementacao real depende da spike de captura (docs/spikes/).

use serde::Serialize;

use crate::error::{Error, Result};

/// Frame capturado do monitor onde esta a janela em foco.
#[derive(Debug, Serialize)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Escala de DPI do monitor — a UI precisa dela para posicionar as bboxes.
    pub scale_factor: f64,
    /// Titulo da janela em foco no momento da captura (nome do jogo).
    pub window_title: Option<String>,
    /// Pixels BGRA8, `width * height * 4` bytes.
    #[serde(skip)]
    pub pixels: Vec<u8>,
}

/// Captura o monitor onde esta a janela atualmente em foco.
pub fn capture_focused_monitor() -> Result<Frame> {
    Err(Error::NotImplemented("capture::capture_focused_monitor"))
}
