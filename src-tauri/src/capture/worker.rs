//! Thread dedicada a captura.
//!
//! Existe por uma razao concreta: as interfaces COM do crate `windows` nao
//! implementam `Send`, entao o device D3D11 e o item de captura **nao podem**
//! ser guardados num `static` compartilhado nem viajar entre as threads do
//! runtime do Tauri. Recriar tudo a cada captura custaria dezenas de
//! milissegundos de um orcamento de 1 s.
//!
//! Aqui os objetos COM ficam presos a uma unica thread e o que atravessa o
//! canal e so o [`Shot`] — numeros e um `Vec<u8>`, que sao `Send`.

use std::sync::mpsc::{channel, sync_channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use super::wgc::{Capturer, Shot};
use super::Region;
use crate::error::{Error, Result};
use crate::platform::MonitorRect;

/// Teto da espera pela thread de captura.
///
/// A captura em si ja tem timeout proprio (1 s para o primeiro frame); este
/// cobre o resto — criacao de device, driver travado — para que um lookup nunca
/// deixe a UI pendurada.
const RESPOSTA_TIMEOUT: Duration = Duration::from_secs(5);

struct Pedido {
    hmonitor: isize,
    monitor: MonitorRect,
    regiao: Region,
    resposta: std::sync::mpsc::SyncSender<Result<Shot>>,
}

static FILA: OnceLock<Mutex<Sender<Pedido>>> = OnceLock::new();

/// Captura um recorte do monitor `hmonitor`, ligando a thread na primeira vez.
pub fn capture(hmonitor: isize, monitor: MonitorRect, regiao: Region) -> Result<Shot> {
    let (tx, rx) = sync_channel::<Result<Shot>>(1);
    let pedido = Pedido {
        hmonitor,
        monitor,
        regiao,
        resposta: tx,
    };

    fila()
        .lock()
        .map_err(|_| Error::Platform("fila de captura envenenada".into()))?
        .send(pedido)
        .map_err(|_| Error::Platform("thread de captura morreu".into()))?;

    rx.recv_timeout(RESPOSTA_TIMEOUT).map_err(|_| {
        Error::Platform(format!(
            "thread de captura nao respondeu em {} s",
            RESPOSTA_TIMEOUT.as_secs()
        ))
    })?
}

fn fila() -> &'static Mutex<Sender<Pedido>> {
    FILA.get_or_init(|| {
        let (tx, rx) = channel::<Pedido>();
        std::thread::Builder::new()
            .name("papaplay-capture".into())
            .spawn(move || atender(rx))
            .expect("nao foi possivel criar a thread de captura");
        Mutex::new(tx)
    })
}

fn atender(rx: Receiver<Pedido>) {
    // As APIs WinRT exigem um apartment COM inicializado. MTA e o certo aqui:
    // um apartment single-threaded precisaria de um loop de mensagens, que esta
    // thread nao tem. Erro so acontece se a thread ja estivesse em outro
    // apartment, o que nao e o caso numa thread recem-criada.
    // SAFETY: chamada de inicializacao padrao, sem ponteiros de entrada.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    let mut capturer: Option<Capturer> = None;
    for pedido in rx {
        let resultado = uma_captura(&mut capturer, &pedido);
        // Receptor sumido = quem pediu desistiu (timeout). Nao e erro.
        let _ = pedido.resposta.send(resultado);
    }
}

fn uma_captura(capturer: &mut Option<Capturer>, pedido: &Pedido) -> Result<Shot> {
    // Alt-tab para um jogo no outro monitor troca o alvo: o item de captura e
    // amarrado a um HMONITOR e precisa ser refeito.
    if capturer.as_ref().map(Capturer::hmonitor) != Some(pedido.hmonitor) {
        *capturer = None;
    }

    if capturer.is_none() {
        *capturer = Some(Capturer::new(pedido.hmonitor)?);
    }

    let resultado = capturer
        .as_ref()
        .expect("o capturer acabou de ser criado")
        .grab(pedido.monitor, pedido.regiao);

    if resultado.is_err() {
        // Device perdido (reinicio de driver, troca de GPU) da erro em toda
        // captura seguinte se for reaproveitado. Descartar aqui faz a proxima
        // tentativa recriar tudo em vez de falhar para sempre.
        *capturer = None;
    }
    resultado
}
