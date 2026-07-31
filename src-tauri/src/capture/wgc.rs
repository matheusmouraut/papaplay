//! Implementacao da captura em Windows Graphics Capture + D3D11.
//!
//! Nada aqui atravessa threads: as interfaces COM do crate `windows` nao sao
//! `Send`, e e por isso que [`super::worker`] existe. Todo este modulo roda na
//! thread `papaplay-capture`.
//!
//! O caminho de um frame:
//!
//! ```text
//! HMONITOR -> GraphicsCaptureItem -> Direct3D11CaptureFramePool
//!   -> IDirect3DSurface -> ID3D11Texture2D (na GPU)
//!   -> CopySubresourceRegion (recorta ja na GPU)
//!   -> textura STAGING -> Map -> Vec<u8> BGRA8
//! ```
//!
//! Recortar antes da leitura de volta importa: copiar 1280x720 em vez de
//! 2560x1440 e um quarto do trafego pela ponte GPU->CPU.

use std::sync::mpsc::sync_channel;
use std::time::Duration;

use windows::core::{Interface, Ref};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BOX,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

use super::Region;
use crate::error::{Error, Result};
use crate::platform::MonitorRect;

/// Quanto esperar pelo primeiro frame depois de `StartCapture`.
///
/// Na pratica ele chega em uma composicao (~8 ms a 120 Hz). O teto generoso e
/// para o caso de o compositor estar engasgado com o jogo — estourar aqui vira
/// erro com mensagem, nunca travamento da thread.
const FRAME_TIMEOUT: Duration = Duration::from_millis(1000);

/// Buffers do frame pool. Dois em vez de um: com apenas um, o pool pode
/// reciclar o buffer entre o evento e o `TryGetNextFrame` e devolver vazio.
const POOL_BUFFERS: i32 = 2;

/// Retangulo efetivamente copiado, ja em coordenadas da area virtual.
pub struct Shot {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// BGRA8, `width * height * 4` bytes, sem padding.
    pub pixels: Vec<u8>,
}

fn win_err(contexto: &str, e: windows::core::Error) -> Error {
    Error::Platform(format!("{contexto}: {e}"))
}

/// Device D3D11 + item de captura de um monitor.
///
/// Criar custa dezenas de milissegundos, entao vive enquanto o monitor alvo
/// nao mudar. O que **nao** fica vivo e a sessao de captura — ver o modulo
/// [`super`] para o porque.
pub struct Capturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    /// Mesmo device, na forma que a API WinRT aceita.
    winrt_device: windows::Graphics::DirectX::Direct3D11::IDirect3DDevice,
    hmonitor: isize,
    item: GraphicsCaptureItem,
}

impl Capturer {
    pub fn new(hmonitor: isize) -> Result<Self> {
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            return Err(Error::Platform(
                "Windows Graphics Capture nao esta disponivel nesta maquina".into(),
            ));
        }

        let (device, context) = criar_device()?;
        let dxgi: IDXGIDevice = device
            .cast()
            .map_err(|e| win_err("ID3D11Device para IDXGIDevice", e))?;
        // SAFETY: `dxgi` e um device valido recem-criado; a funcao devolve uma
        // referencia nova que o `cast` abaixo converte sem consumir.
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
            .map_err(|e| win_err("CreateDirect3D11DeviceFromDXGIDevice", e))?;
        let winrt_device = inspectable
            .cast()
            .map_err(|e| win_err("IInspectable para IDirect3DDevice", e))?;

        let item = item_do_monitor(hmonitor)?;

        Ok(Self {
            device,
            context,
            winrt_device,
            hmonitor,
            item,
        })
    }

    pub fn hmonitor(&self) -> isize {
        self.hmonitor
    }

    /// Captura um frame e devolve so o recorte pedido.
    ///
    /// `monitor` e o retangulo do monitor na area virtual: e ele que traduz a
    /// regiao (coordenadas de tela) para coordenadas da textura.
    pub fn grab(&self, monitor: MonitorRect, regiao: Region) -> Result<Shot> {
        let tamanho = self
            .item
            .Size()
            .map_err(|e| win_err("GraphicsCaptureItem::Size", e))?;

        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &self.winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            POOL_BUFFERS,
            tamanho,
        )
        .map_err(|e| win_err("CreateFreeThreaded", e))?;

        let sessao = pool
            .CreateCaptureSession(&self.item)
            .map_err(|e| win_err("CreateCaptureSession", e))?;

        // O cursor por cima de uma palavra atrapalha o OCR, e ninguem quer ler
        // o proprio ponteiro.
        let _ = sessao.SetIsCursorCaptureEnabled(false);
        // A borda amarela do WGC so pode ser desligada em builds recentes do
        // Windows 11; falhar aqui e cosmetico, nao impede a captura.
        let _ = sessao.SetIsBorderRequired(false);

        let (tx, rx) = sync_channel::<()>(1);
        let token = pool
            .FrameArrived(&TypedEventHandler::<
                Direct3D11CaptureFramePool,
                windows::core::IInspectable,
            >::new(
                move |_pool: Ref<'_, Direct3D11CaptureFramePool>, _args| {
                    // `try_send` de proposito: so interessa o primeiro aviso, e o
                    // callback nunca pode bloquear a thread do compositor.
                    let _ = tx.try_send(());
                    Ok(())
                },
            ))
            .map_err(|e| win_err("FrameArrived", e))?;

        sessao
            .StartCapture()
            .map_err(|e| win_err("StartCapture", e))?;

        let resultado = self.primeiro_frame(&pool, &rx, monitor, regiao);

        // Fecha na ordem inversa da criacao, sempre — inclusive no caminho de
        // erro, senao a sessao continuaria produzindo frames.
        let _ = pool.RemoveFrameArrived(token);
        let _ = sessao.Close();
        let _ = pool.Close();

        resultado
    }

    fn primeiro_frame(
        &self,
        pool: &Direct3D11CaptureFramePool,
        rx: &std::sync::mpsc::Receiver<()>,
        monitor: MonitorRect,
        regiao: Region,
    ) -> Result<Shot> {
        if rx.recv_timeout(FRAME_TIMEOUT).is_err() {
            return Err(Error::Platform(format!(
                "nenhum frame em {} ms — o compositor nao entregou a tela",
                FRAME_TIMEOUT.as_millis()
            )));
        }

        let frame = pool
            .TryGetNextFrame()
            .map_err(|e| win_err("TryGetNextFrame", e))?;
        let surface = frame
            .Surface()
            .map_err(|e| win_err("Direct3D11CaptureFrame::Surface", e))?;
        let acesso: IDirect3DDxgiInterfaceAccess = surface
            .cast()
            .map_err(|e| win_err("IDirect3DSurface para IDirect3DDxgiInterfaceAccess", e))?;
        // SAFETY: a surface veio do frame pool com formato BGRA8, entao a
        // interface subjacente e sempre uma ID3D11Texture2D.
        let textura: ID3D11Texture2D = unsafe { acesso.GetInterface() }
            .map_err(|e| win_err("IDirect3DDxgiInterfaceAccess::GetInterface", e))?;

        let shot = self.ler(&textura, monitor, regiao);
        let _ = frame.Close();
        shot
    }

    fn ler(&self, textura: &ID3D11Texture2D, monitor: MonitorRect, regiao: Region) -> Result<Shot> {
        let mut origem = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: `GetDesc` so escreve na struct de saida.
        unsafe { textura.GetDesc(&mut origem) };

        let caixa = recortar(monitor, regiao, origem.Width, origem.Height)?;
        let width = caixa.right - caixa.left;
        let height = caixa.bottom - caixa.top;

        let destino = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: origem.Format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };

        let mut staging: Option<ID3D11Texture2D> = None;
        // SAFETY: `destino` descreve uma textura valida e `staging` recebe a
        // saida; sem dados iniciais.
        unsafe {
            self.device
                .CreateTexture2D(&destino, None, Some(&mut staging))
        }
        .map_err(|e| win_err("CreateTexture2D (staging)", e))?;
        let staging = staging.ok_or_else(|| {
            Error::Platform("CreateTexture2D devolveu sucesso sem textura".into())
        })?;

        // SAFETY: as duas texturas tem o mesmo formato, e `caixa` foi limitada
        // as dimensoes reais da textura de origem por `recortar`.
        unsafe {
            self.context
                .CopySubresourceRegion(&staging, 0, 0, 0, 0, textura, 0, Some(&caixa));
        }

        let mut mapeado = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: a textura e STAGING com CPU_ACCESS_READ, entao Map e valido.
        unsafe {
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapeado))
        }
        .map_err(|e| win_err("Map (staging)", e))?;

        let pixels = copiar_linhas(&mapeado, width, height);

        // SAFETY: par obrigatorio do Map acima, mesma textura e subresource.
        unsafe { self.context.Unmap(&staging, 0) };

        Ok(Shot {
            x: monitor.x + caixa.left as i32,
            y: monitor.y + caixa.top as i32,
            width,
            height,
            pixels,
        })
    }
}

/// Copia respeitando o `RowPitch`: a GPU alinha cada linha, entao o buffer
/// mapeado quase nunca tem exatamente `width * 4` bytes por linha.
fn copiar_linhas(mapeado: &D3D11_MAPPED_SUBRESOURCE, width: u32, height: u32) -> Vec<u8> {
    let bytes_por_linha = width as usize * 4;
    let pitch = mapeado.RowPitch as usize;
    let mut pixels = vec![0u8; bytes_por_linha * height as usize];

    for y in 0..height as usize {
        // SAFETY: `pData` aponta para `pitch * height` bytes validos enquanto o
        // Map estiver ativo, e cada linha lida cabe dentro do pitch.
        let linha = unsafe {
            std::slice::from_raw_parts((mapeado.pData as *const u8).add(y * pitch), bytes_por_linha)
        };
        pixels[y * bytes_por_linha..(y + 1) * bytes_por_linha].copy_from_slice(linha);
    }
    pixels
}

/// Traduz a regiao (coordenadas de tela) para uma caixa dentro da textura.
///
/// Limita as dimensoes reais da textura: se o retangulo passasse do fim,
/// `CopySubresourceRegion` falharia em silencio e a captura sairia preta.
fn recortar(
    monitor: MonitorRect,
    regiao: Region,
    tex_width: u32,
    tex_height: u32,
) -> Result<D3D11_BOX> {
    let left = (regiao.x - monitor.x).max(0) as u32;
    let top = (regiao.y - monitor.y).max(0) as u32;
    let left = left.min(tex_width);
    let top = top.min(tex_height);
    let right = left.saturating_add(regiao.width).min(tex_width);
    let bottom = top.saturating_add(regiao.height).min(tex_height);

    if right <= left || bottom <= top {
        return Err(Error::Platform(format!(
            "regiao {regiao:?} nao intersecta a textura de {tex_width}x{tex_height}"
        )));
    }

    Ok(D3D11_BOX {
        left,
        top,
        front: 0,
        right,
        bottom,
        back: 1,
    })
}

fn criar_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    // WARP e o plano B por software: maquinas sem GPU compativel (ou com driver
    // em estado ruim) ainda conseguem capturar, mais devagar.
    match device_com_driver(D3D_DRIVER_TYPE_HARDWARE) {
        Ok(par) => Ok(par),
        Err(_) => device_com_driver(D3D_DRIVER_TYPE_WARP),
    }
}

fn device_com_driver(driver: D3D_DRIVER_TYPE) -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    // SAFETY: chamada padrao de criacao de device; os dois `Option` de saida
    // sao locais validos e o resto sao valores por copia.
    unsafe {
        D3D11CreateDevice(
            None,
            driver,
            HMODULE::default(),
            // BGRA_SUPPORT e requisito da interop com WinRT.
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .map_err(|e| win_err("D3D11CreateDevice", e))?;

    match (device, context) {
        (Some(device), Some(context)) => Ok((device, context)),
        _ => Err(Error::Platform(
            "D3D11CreateDevice devolveu sucesso sem device".into(),
        )),
    }
}

fn item_do_monitor(hmonitor: isize) -> Result<GraphicsCaptureItem> {
    let interop: IGraphicsCaptureItemInterop =
        windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(|e| win_err("fabrica de GraphicsCaptureItem", e))?;
    // SAFETY: o handle veio de `MonitorFromWindow` no mesmo processo; a API
    // valida e devolve erro para handles invalidos.
    unsafe { interop.CreateForMonitor(HMONITOR(hmonitor as *mut core::ffi::c_void)) }
        .map_err(|e| win_err("CreateForMonitor", e))
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

    #[test]
    fn recorte_no_meio_vira_caixa_local_da_textura() {
        let regiao = Region {
            x: 640,
            y: 360,
            width: 1280,
            height: 720,
        };
        let caixa = recortar(monitor(), regiao, 2560, 1440).expect("recorte valido");
        assert_eq!((caixa.left, caixa.top), (640, 360));
        assert_eq!((caixa.right, caixa.bottom), (1920, 1080));
        assert_eq!((caixa.front, caixa.back), (0, 1));
    }

    #[test]
    fn recorte_em_monitor_secundario_fica_relativo_a_textura() {
        // A textura do WGC comeca no canto do monitor, nao no canto da area
        // virtual: um monitor em x = -1920 tem que virar left = 0.
        let secundario = MonitorRect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let regiao = Region {
            x: -1920,
            y: 0,
            width: 1280,
            height: 720,
        };
        let caixa = recortar(secundario, regiao, 1920, 1080).expect("recorte valido");
        assert_eq!((caixa.left, caixa.top), (0, 0));
        assert_eq!((caixa.right, caixa.bottom), (1280, 720));
    }

    #[test]
    fn textura_menor_que_o_monitor_reportado_corta_em_vez_de_estourar() {
        // Acontece se o GetMonitorInfoW e a textura discordarem; sem o limite,
        // CopySubresourceRegion falharia em silencio e a imagem sairia preta.
        let regiao = Region {
            x: 2000,
            y: 1000,
            width: 1280,
            height: 720,
        };
        let caixa = recortar(monitor(), regiao, 2048, 1152).expect("recorte valido");
        assert_eq!((caixa.right, caixa.bottom), (2048, 1152));
        assert!(caixa.right > caixa.left && caixa.bottom > caixa.top);
    }

    #[test]
    fn regiao_totalmente_fora_da_textura_e_erro() {
        let regiao = Region {
            x: 5000,
            y: 5000,
            width: 1280,
            height: 720,
        };
        assert!(recortar(monitor(), regiao, 2560, 1440).is_err());
    }

    #[test]
    fn copia_de_linhas_descarta_o_padding_do_pitch() {
        // 2x2 pixels com pitch de 12 bytes (8 uteis + 4 de padding).
        let bruto: Vec<u8> = vec![
            1, 2, 3, 4, 5, 6, 7, 8, 0xAA, 0xAA, 0xAA, 0xAA, // linha 0 + padding
            9, 10, 11, 12, 13, 14, 15, 16, 0xBB, 0xBB, 0xBB, 0xBB, // linha 1 + padding
        ];
        let mapeado = D3D11_MAPPED_SUBRESOURCE {
            pData: bruto.as_ptr() as *mut core::ffi::c_void,
            RowPitch: 12,
            DepthPitch: 24,
        };
        let pixels = copiar_linhas(&mapeado, 2, 2);
        assert_eq!(
            pixels,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
    }

    #[test]
    fn copia_de_linhas_sem_padding_e_copia_direta() {
        let bruto: Vec<u8> = (0..16).collect();
        let mapeado = D3D11_MAPPED_SUBRESOURCE {
            pData: bruto.as_ptr() as *mut core::ffi::c_void,
            RowPitch: 8,
            DepthPitch: 16,
        };
        assert_eq!(copiar_linhas(&mapeado, 2, 2), bruto);
    }
}
