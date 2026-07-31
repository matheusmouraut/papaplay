//! Consultas ao Windows sobre janelas e monitores.
//!
//! REGRA INVIOLAVEL: aqui so entram APIs de *leitura* de estado de janelas
//! alheias (`GetForegroundWindow`, `MonitorFromWindow`, `GetWindowTextW`) e
//! `SetForegroundWindow` para devolver o foco. Nada de hook, injecao de DLL ou
//! escrita na memoria de outro processo — ver CLAUDE.md.

use serde::Serialize;

/// Retangulo de um monitor em pixels fisicos, em coordenadas da area de
/// trabalho virtual (o monitor secundario pode ter `left`/`top` negativos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Janela que estava em primeiro plano e o monitor que a contem.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundTarget {
    /// Handle da janela como inteiro — so serve para devolver o foco depois.
    #[serde(skip)]
    pub hwnd: isize,
    /// Titulo da janela em foco (= nome do jogo, na pratica).
    pub window_title: Option<String>,
    pub monitor: MonitorRect,
}

#[cfg(windows)]
mod imp {
    use std::mem::size_of;

    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, SetForegroundWindow,
    };

    use super::{ForegroundTarget, MonitorRect};
    use crate::error::{Error, Result};

    fn to_hwnd(raw: isize) -> HWND {
        HWND(raw as *mut core::ffi::c_void)
    }

    fn window_title(hwnd: HWND) -> Option<String> {
        // SAFETY: `hwnd` acabou de vir de GetForegroundWindow; as duas chamadas
        // sao read-only e toleram handles invalidos (retornam 0).
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let written = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if written <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..written as usize]))
    }

    fn monitor_rect(hwnd: HWND) -> Result<MonitorRect> {
        // Sem janela em foco (ex.: desktop vazio) caimos no monitor primario.
        let flags = if hwnd.0.is_null() {
            MONITOR_DEFAULTTOPRIMARY
        } else {
            MONITOR_DEFAULTTONEAREST
        };
        // SAFETY: ambas as chamadas sao read-only; `info` tem cbSize preenchido
        // como a API exige e vive por toda a chamada.
        let monitor = unsafe { MonitorFromWindow(hwnd, flags) };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let ok = unsafe { GetMonitorInfoW(monitor, &mut info) };
        if !ok.as_bool() {
            return Err(Error::Platform("GetMonitorInfoW retornou FALSE".into()));
        }
        let RECT {
            left,
            top,
            right,
            bottom,
        } = info.rcMonitor;
        Ok(MonitorRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }

    /// Janela em primeiro plano *agora* e o monitor dela.
    pub fn foreground_target() -> Result<ForegroundTarget> {
        // SAFETY: leitura de estado global do shell, sem argumentos.
        let hwnd = unsafe { GetForegroundWindow() };
        Ok(ForegroundTarget {
            hwnd: hwnd.0 as isize,
            window_title: window_title(hwnd),
            monitor: monitor_rect(hwnd)?,
        })
    }

    /// Devolve o foco para a janela que o tinha antes do modo lookup.
    ///
    /// Best-effort: o Windows recusa `SetForegroundWindow` quando o processo
    /// chamador nao esta em primeiro plano, entao a falha nao e um erro fatal.
    pub fn restore_foreground(hwnd: isize) -> bool {
        if hwnd == 0 {
            return false;
        }
        // SAFETY: handle possivelmente obsoleto e aceito — a API valida e
        // retorna FALSE em vez de causar UB.
        unsafe { SetForegroundWindow(to_hwnd(hwnd)).as_bool() }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::ForegroundTarget;
    use crate::error::{Error, Result};

    pub fn foreground_target() -> Result<ForegroundTarget> {
        Err(Error::Platform("suportado apenas no Windows".into()))
    }

    pub fn restore_foreground(_hwnd: isize) -> bool {
        false
    }
}

pub use imp::{foreground_target, restore_foreground};

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn foreground_target_devolve_um_monitor_com_area_positiva() {
        // Roda sem sessao interativa no CI: sem janela em foco a API cai no
        // monitor primario, entao o retangulo tem que ser valido de qualquer jeito.
        let target = foreground_target().expect("consulta de monitor deve funcionar no Windows");
        assert!(target.monitor.width > 0, "largura: {:?}", target.monitor);
        assert!(target.monitor.height > 0, "altura: {:?}", target.monitor);
    }

    #[test]
    fn restaurar_foco_de_handle_nulo_e_no_op() {
        assert!(!restore_foreground(0));
    }
}
