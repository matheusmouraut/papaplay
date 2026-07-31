//! Ferramenta de verificacao da captura: fotografa a tela, mede e salva um PNG.
//!
//! ```powershell
//! # janela de 1280x720 em volta do cursor, o que o lookup vai fazer
//! cargo run --release --bin capture-probe
//!
//! # espera 5 s para dar tempo de alt-tab para o jogo, depois captura 10 vezes
//! cargo run --release --bin capture-probe -- --espera 5 --repeticoes 10
//!
//! # tela inteira, e roda o OCR em cima do que foi capturado
//! cargo run --release --bin capture-probe -- --tela-inteira --ocr
//! ```
//!
//! Um PNG preto aqui significa captura falhando de verdade — a maioria dos
//! problemas de WGC nao aparece como erro, aparece como imagem vazia.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use papaplay_lib::capture::{self, Frame, LOOKUP_REGION};

struct Args {
    /// `LARGURAxALTURA` da janela em volta do cursor.
    regiao: (u32, u32),
    tela_inteira: bool,
    repeticoes: usize,
    espera: u64,
    ocr: bool,
    saida: PathBuf,
}

fn parse_args() -> Args {
    let mut args = Args {
        regiao: LOOKUP_REGION,
        tela_inteira: false,
        repeticoes: 1,
        espera: 0,
        ocr: false,
        saida: PathBuf::from("capture-probe.png"),
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--regiao" => {
                if let Some(r) = iter.next().as_deref().and_then(parse_regiao) {
                    args.regiao = r;
                }
            }
            "--tela-inteira" => args.tela_inteira = true,
            "--repeticoes" => {
                args.repeticoes = iter.next().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
            }
            "--espera" => args.espera = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--ocr" => args.ocr = true,
            "--saida" => {
                if let Some(p) = iter.next() {
                    args.saida = PathBuf::from(p);
                }
            }
            outro => eprintln!("argumento ignorado: {outro}"),
        }
    }
    args
}

fn parse_regiao(bruto: &str) -> Option<(u32, u32)> {
    let (w, h) = bruto.split_once(['x', 'X'])?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

fn capturar(args: &Args) -> Result<Frame, Box<dyn std::error::Error>> {
    if args.tela_inteira {
        Ok(capture::capture_focused_monitor()?)
    } else {
        Ok(capture::capture_focused_region(args.regiao, None)?)
    }
}

/// Fracao de pixels nao pretos.
///
/// O sintoma classico de WGC quebrado e um frame valido em tamanho e todo
/// preto; esta e a checagem mais barata que separa isso de uma captura boa.
fn fracao_com_conteudo(frame: &Frame) -> f64 {
    let total = (frame.width * frame.height) as usize;
    if total == 0 {
        return 0.0;
    }
    let nao_pretos = frame
        .pixels
        .chunks_exact(4)
        .filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8)
        .count();
    nao_pretos as f64 / total as f64
}

fn salvar(frame: &Frame, destino: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut rgb = image::RgbImage::new(frame.width, frame.height);
    for (i, pixel) in rgb.pixels_mut().enumerate() {
        let b = frame.pixels[i * 4];
        let g = frame.pixels[i * 4 + 1];
        let r = frame.pixels[i * 4 + 2];
        *pixel = image::Rgb([r, g, b]);
    }
    rgb.save(destino)?;
    Ok(())
}

/// Working set do processo em MB. Fecha a duvida de RAM que a spike 01 deixou:
/// com `--ocr`, a diferenca antes/depois de carregar os modelos e o custo real
/// do runtime ONNX.
#[cfg(windows)]
fn ram_mb() -> f64 {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;

    let mut contadores = PROCESS_MEMORY_COUNTERS::default();
    let tamanho = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: leitura de contadores do proprio processo; a struct de saida e
    // um local valido e `tamanho` descreve o layout que a API espera.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut contadores, tamanho) };
    if ok.is_err() {
        return 0.0;
    }
    contadores.WorkingSetSize as f64 / (1024.0 * 1024.0)
}

#[cfg(not(windows))]
fn ram_mb() -> f64 {
    0.0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();

    if args.espera > 0 {
        println!(
            "esperando {} s — passe o foco para o jogo agora",
            args.espera
        );
        std::thread::sleep(Duration::from_secs(args.espera));
    }

    println!("ram antes de qualquer captura: {:.1} MB", ram_mb());

    let mut tempos: Vec<Duration> = Vec::with_capacity(args.repeticoes);
    let mut ultimo: Option<Frame> = None;

    for i in 0..args.repeticoes {
        let comeco = Instant::now();
        let frame = capturar(&args)?;
        let levou = comeco.elapsed();
        tempos.push(levou);

        println!(
            "{:>3}. {}x{} em ({}, {})  escala {:.2}  {:>7.1} ms  conteudo {:.1}%  janela: {}",
            i + 1,
            frame.width,
            frame.height,
            frame.x,
            frame.y,
            frame.scale_factor,
            levou.as_secs_f64() * 1000.0,
            fracao_com_conteudo(&frame) * 100.0,
            frame.window_title.as_deref().unwrap_or("(sem titulo)"),
        );
        ultimo = Some(frame);
    }

    // A primeira captura paga a criacao do device D3D11; e o unico numero que
    // nao se repete, entao vale separado das outras.
    let primeira = tempos[0];
    tempos.sort();
    let media: f64 =
        tempos.iter().map(|d| d.as_secs_f64()).sum::<f64>() / tempos.len() as f64 * 1000.0;
    println!(
        "\nprimeira {:.1} ms (device frio) | mediana {:.1} ms | pior {:.1} ms | media {:.1} ms",
        primeira.as_secs_f64() * 1000.0,
        tempos[tempos.len() / 2].as_secs_f64() * 1000.0,
        tempos.last().expect("ao menos uma amostra").as_secs_f64() * 1000.0,
        media,
    );
    println!("ram depois das capturas: {:.1} MB", ram_mb());

    let frame = ultimo.expect("ao menos uma repeticao");
    salvar(&frame, &args.saida)?;
    println!("imagem salva em {}", args.saida.display());

    if args.ocr {
        let modelos = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/models");
        println!("\ncarregando modelos de {}", modelos.display());
        let comeco = Instant::now();
        let mut engine = papaplay_lib::ocr::Engine::load(&modelos)?;
        println!(
            "modelos prontos em {:?} | ram: {:.1} MB",
            comeco.elapsed(),
            ram_mb()
        );

        let comeco = Instant::now();
        let resultado = papaplay_lib::ocr::recognize(&mut engine, &frame)?;
        println!(
            "ocr em {:.1} ms: {} palavras em {} linhas | ram: {:.1} MB\n",
            comeco.elapsed().as_secs_f64() * 1000.0,
            resultado.words.len(),
            resultado.lines.len(),
            ram_mb(),
        );
        for linha in &resultado.lines {
            println!("  {}", linha.text);
        }
    }

    Ok(())
}
