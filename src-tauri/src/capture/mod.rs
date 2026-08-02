//! Captura de tela via Windows Graphics Capture (WGC).
//!
//! REGRA INVIOLAVEL: captura externa apenas. Nunca injetar DLL nem instalar
//! hook no processo do jogo (risco de anti-cheat). WGC roda inteiro do lado do
//! compositor do Windows — o jogo nao e tocado. Ver CLAUDE.md, regra 1.
//!
//! # Por que uma thread dedicada
//!
//! As interfaces COM do crate `windows` nao sao `Send`, entao o device D3D11
//! nao pode viajar entre threads. Criar um device por captura custa dezenas de
//! milissegundos. A solucao e [`worker`]: uma thread so, dona do device, que
//! recebe pedidos por canal e devolve pixels (que sao `Send`).
//!
//! # Por que a sessao de captura nao fica ligada
//!
//! Uma `GraphicsCaptureSession` viva entrega um frame por composicao — com um
//! jogo a 120 fps sao 120 callbacks por segundo com o overlay parado. Isso
//! quebraria a regra 5 (overlay passivo ~0% CPU). O device fica quente, a
//! sessao nasce e morre a cada captura.
//!
//! # Por que so um pedaco da tela
//!
//! A spike 02 mediu o custo do OCR em `~60 ms + 11 ms por caixa de texto`: ler
//! a tela 2.5K inteira chega a 720 ms, e uma janela de 1280x720 em volta do
//! cursor fica em 252 ms no pior caso. Ver `docs/spikes/spike-ocr-resultado.md`.
//! Dai [`LOOKUP_REGION`] e [`capture_focused_region`].

#[cfg(windows)]
mod wgc;
#[cfg(windows)]
mod worker;

use serde::Serialize;

use crate::error::Result;
use crate::platform::{self, MonitorRect};

/// Tamanho padrao da janela de OCR em volta do cursor, em pixels fisicos.
///
/// Vem da medicao da spike 02: e o maior recorte que mantem o pior caso do OCR
/// abaixo de ~250 ms, deixando ~750 ms do orcamento de 1 s do lookup (doc 03)
/// para dicionario e traducao.
pub const LOOKUP_REGION: (u32, u32) = (1280, 720);

/// Retangulo pedido a captura, em coordenadas da area de trabalho virtual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl From<MonitorRect> for Region {
    fn from(monitor: MonitorRect) -> Self {
        Region {
            x: monitor.x,
            y: monitor.y,
            width: monitor.width.max(0) as u32,
            height: monitor.height.max(0) as u32,
        }
    }
}

/// Frame capturado do monitor onde esta a janela em foco.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Canto superior esquerdo do frame na area de trabalho virtual. As bboxes
    /// do OCR saem relativas ao frame; somar `(x, y)` da a posicao na tela, que
    /// e o que a overlay precisa para desenhar em cima da palavra certa.
    pub x: i32,
    pub y: i32,
    /// Escala de DPI do monitor — a UI precisa dela para posicionar as bboxes.
    pub scale_factor: f64,
    /// Titulo da janela em foco no momento da captura (nome do jogo).
    pub window_title: Option<String>,
    /// Pixels BGRA8, `width * height * 4` bytes.
    #[serde(skip)]
    pub pixels: Vec<u8>,
}

/// Janela de `size` centrada em `center`, empurrada para dentro do monitor.
///
/// Perto da borda a janela **desliza** em vez de encolher: o OCR prefere ler
/// 1280x720 deslocados a ler um retangulo cortado, porque o texto de interface
/// costuma ficar justamente nas bordas. Se o monitor for menor que `size`, o
/// resultado e o monitor inteiro.
pub fn region_around(monitor: MonitorRect, center: (i32, i32), size: (u32, u32)) -> Region {
    let disponivel = Region::from(monitor);
    let width = size.0.min(disponivel.width);
    let height = size.1.min(disponivel.height);

    let max_x = disponivel.x + (disponivel.width - width) as i32;
    let max_y = disponivel.y + (disponivel.height - height) as i32;
    let x = (center.0 - width as i32 / 2).clamp(disponivel.x, max_x);
    let y = (center.1 - height as i32 / 2).clamp(disponivel.y, max_y);

    Region {
        x,
        y,
        width,
        height,
    }
}

/// Prepara a captura do monitor em foco antes da primeira espiada.
///
/// O device D3D11 e o item de captura custam ~300 ms para nascer e depois ficam
/// vivos na thread de captura. Pagar isso no boot, em segundo plano, e o que
/// tira esse tempo do primeiro `Alt+X` do usuario.
#[cfg(windows)]
pub fn warm_up() {
    std::thread::spawn(|| {
        if let Ok(alvo) = platform::foreground_target() {
            worker::warm_up(alvo.hmonitor, alvo.monitor);
        }
    });
}

#[cfg(not(windows))]
pub fn warm_up() {}

/// Captura o monitor inteiro onde esta a janela em foco.
///
/// Custa caro no OCR — para o lookup use [`capture_focused_region`].
pub fn capture_focused_monitor() -> Result<Frame> {
    let target = platform::foreground_target()?;
    let regiao = Region::from(target.monitor);
    grab(&target, regiao)
}

/// Captura uma janela de `size` em volta de `center`.
///
/// `center` em `None` usa a posicao atual do cursor; sem cursor (sessao remota,
/// por exemplo) cai no centro do monitor.
pub fn capture_focused_region(size: (u32, u32), center: Option<(i32, i32)>) -> Result<Frame> {
    let target = platform::foreground_target()?;
    capture_region_of(&target, size, center)
}

/// Igual a [`capture_focused_region`], mas num alvo ja conhecido.
///
/// E o que a consulta usa: em modo lookup a janela em foco e a **propria
/// overlay**, entao perguntar ao Windows de novo devolveria o alvo errado. O
/// alvo bom e o congelado por `overlay::set_mode`.
pub fn capture_region_of(
    target: &platform::ForegroundTarget,
    size: (u32, u32),
    center: Option<(i32, i32)>,
) -> Result<Frame> {
    let center = center
        .or_else(platform::cursor_pos)
        .unwrap_or_else(|| centro(target.monitor));
    let regiao = region_around(target.monitor, center, size);
    grab(target, regiao)
}

fn centro(monitor: MonitorRect) -> (i32, i32) {
    (
        monitor.x + monitor.width / 2,
        monitor.y + monitor.height / 2,
    )
}

#[cfg(windows)]
fn grab(target: &platform::ForegroundTarget, regiao: Region) -> Result<Frame> {
    // A captura devolve o retangulo de fato copiado: a textura do WGC pode nao
    // bater com o retangulo que o GetMonitorInfoW reportou, e nesse caso vale o
    // que veio da textura, senao as bboxes sairiam deslocadas.
    let shot = worker::capture(target.hmonitor, target.monitor, regiao)?;
    Ok(Frame {
        width: shot.width,
        height: shot.height,
        x: shot.x,
        y: shot.y,
        scale_factor: target.scale_factor,
        window_title: target.window_title.clone(),
        pixels: shot.pixels,
    })
}

#[cfg(not(windows))]
fn grab(_target: &platform::ForegroundTarget, _regiao: Region) -> Result<Frame> {
    Err(crate::error::Error::Platform(
        "captura suportada apenas no Windows".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2560x1440 no primario, como a maquina da spike.
    fn monitor() -> MonitorRect {
        MonitorRect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        }
    }

    #[test]
    fn regiao_no_meio_da_tela_fica_centrada_no_ponto() {
        let r = region_around(monitor(), (1280, 720), LOOKUP_REGION);
        assert_eq!(
            r,
            Region {
                x: 640,
                y: 360,
                width: 1280,
                height: 720
            }
        );
    }

    #[test]
    fn regiao_perto_da_borda_desliza_e_nao_encolhe() {
        let r = region_around(monitor(), (10, 10), LOOKUP_REGION);
        assert_eq!(r.width, 1280, "largura tem que ser preservada");
        assert_eq!(r.height, 720, "altura tem que ser preservada");
        assert_eq!((r.x, r.y), (0, 0), "gruda no canto em vez de sair da tela");
    }

    #[test]
    fn regiao_no_canto_oposto_gruda_do_outro_lado() {
        let r = region_around(monitor(), (2559, 1439), LOOKUP_REGION);
        assert_eq!((r.x, r.y), (1280, 720));
        assert_eq!(r.x + r.width as i32, 2560);
        assert_eq!(r.y + r.height as i32, 1440);
    }

    #[test]
    fn cursor_fora_do_monitor_nao_gera_regiao_fora_da_tela() {
        // Acontece de verdade: o cursor pode estar no monitor vizinho no
        // instante do atalho.
        let r = region_around(monitor(), (-5000, 9000), LOOKUP_REGION);
        assert!(r.x >= 0 && r.y >= 0, "{r:?}");
        assert!(r.x + r.width as i32 <= 2560, "{r:?}");
        assert!(r.y + r.height as i32 <= 1440, "{r:?}");
    }

    #[test]
    fn monitor_menor_que_a_janela_pedida_devolve_o_monitor_inteiro() {
        let pequeno = MonitorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let r = region_around(pequeno, (400, 300), LOOKUP_REGION);
        assert_eq!(
            r,
            Region {
                x: 0,
                y: 0,
                width: 800,
                height: 600
            }
        );
    }

    #[test]
    fn monitor_secundario_a_esquerda_mantem_a_origem_negativa() {
        // Monitor a esquerda do primario tem `x` negativo na area virtual; a
        // regiao tem que continuar nesse espaco, nao ser normalizada para zero.
        let secundario = MonitorRect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let r = region_around(secundario, (-960, 540), LOOKUP_REGION);
        assert_eq!((r.x, r.y), (-1600, 180));
        assert!(r.x >= -1920);
        assert!(r.x + r.width as i32 <= 0);
    }

    #[test]
    fn regiao_de_monitor_cobre_o_monitor_inteiro() {
        let r = Region::from(monitor());
        assert_eq!(
            r,
            Region {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440
            }
        );
    }
}
