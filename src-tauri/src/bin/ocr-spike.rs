//! Ferramenta da spike 02: roda o OCR contra as fixtures e mede o resultado.
//!
//! ```powershell
//! # todas as fixtures, com gabarito quando existir
//! cargo run --release --bin ocr-spike
//!
//! # uma imagem so, desenhando as caixas para conferencia visual
//! cargo run --release --bin ocr-spike -- --imagem <caminho> --desenhar
//! ```
//!
//! Sai do repo quando a spike fechar; o que fica e o modulo `ocr`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::{Rgb, RgbImage};
use papaplay_lib::ocr::{Engine, OcrResult};

struct Args {
    imagem: Option<PathBuf>,
    desenhar: bool,
    repeticoes: usize,
    /// `LARGURAxALTURA`: recorta a regiao central antes de rodar o OCR.
    ///
    /// Simula o que o produto realmente faz numa consulta: ler so o entorno do
    /// cursor, nao a tela inteira. Como o custo do reconhecimento e linear no
    /// numero de caixas, esta e a alavanca principal de latencia.
    regiao: Option<(u32, u32)>,
}

fn parse_args() -> Args {
    let mut args = Args {
        imagem: None,
        desenhar: false,
        repeticoes: 1,
        regiao: None,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--imagem" => args.imagem = iter.next().map(PathBuf::from),
            "--desenhar" => args.desenhar = true,
            "--repeticoes" => {
                args.repeticoes = iter.next().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
            }
            "--regiao" => args.regiao = iter.next().and_then(|v| parse_regiao(&v)),
            outro => eprintln!("argumento ignorado: {outro}"),
        }
    }
    args
}

fn parse_regiao(bruto: &str) -> Option<(u32, u32)> {
    let (w, h) = bruto.split_once(['x', 'X'])?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// Recorte centrado, do tamanho pedido (ou a imagem inteira, se ela couber).
fn recortar(imagem: &RgbImage, (largura, altura): (u32, u32)) -> RgbImage {
    let (w, h) = imagem.dimensions();
    let largura = largura.min(w);
    let altura = altura.min(h);
    let x = (w - largura) / 2;
    let y = (h - altura) / 2;
    image::imageops::crop_imm(imagem, x, y, largura, altura).to_image()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri sempre tem pai");
    let modelos = raiz.join("src-tauri/resources/models");
    let fixtures = raiz.join("tests/fixtures/screens");

    println!("carregando modelos de {}", modelos.display());
    let comeco = Instant::now();
    let mut engine = Engine::load(&modelos)?;
    println!("modelos prontos em {:?}\n", comeco.elapsed());

    let imagens = match &args.imagem {
        Some(caminho) => vec![caminho.clone()],
        None => listar(&fixtures)?,
    };
    if imagens.is_empty() {
        println!("nenhuma imagem encontrada em {}", fixtures.display());
        return Ok(());
    }

    let mut relatorio = Vec::new();
    for caminho in &imagens {
        let nome = caminho
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let imagem = image::open(caminho)?.to_rgb8();
        let imagem = match args.regiao {
            Some(alvo) => recortar(&imagem, alvo),
            None => imagem,
        };
        let (w, h) = imagem.dimensions();

        // Primeira passada e "fria": inclui alocacao de buffers do runtime.
        let mut latencias = Vec::new();
        let mut resultado = None;
        for _ in 0..args.repeticoes {
            let t = Instant::now();
            let r = engine.recognize_image(&imagem)?;
            latencias.push(t.elapsed().as_secs_f64() * 1000.0);
            resultado = Some(r);
        }
        let resultado = resultado.expect("ao menos uma repeticao");

        let fria = latencias[0];
        let quente = latencias.iter().skip(1).copied().fold(f64::MAX, f64::min);
        let quente = if quente == f64::MAX { fria } else { quente };

        // Deteccao e uma chamada so; reconhecimento e uma por linha. Separar os
        // dois diz se vale otimizar o modelo ou o numero de chamadas.
        let t = Instant::now();
        let caixas = engine.detect(&imagem)?;
        let det_ms = t.elapsed().as_secs_f64() * 1000.0;

        println!("== {nome}  ({w}x{h})");
        println!(
            "   {} linhas, {} palavras | fria {:.0} ms, quente {:.0} ms \
             (deteccao {:.0} ms + reconhecimento {:.0} ms em {} caixas)",
            resultado.lines.len(),
            resultado.words.len(),
            fria,
            quente,
            det_ms,
            (quente - det_ms).max(0.0),
            caixas.len()
        );

        let gabarito = gabarito_de(caminho);
        let placar = gabarito
            .as_ref()
            .map(|esperadas| avaliar(&resultado, esperadas));
        if let Some(p) = &placar {
            println!(
                "   recall {:.1}% ({}/{}) | faltaram: {}",
                p.recall * 100.0,
                p.acertos,
                p.total,
                if p.faltando.is_empty() {
                    "-".to_string()
                } else {
                    p.faltando.join(", ")
                }
            );
        } else {
            println!("   (sem gabarito .expected.json)");
        }

        for linha in resultado.lines.iter().take(6) {
            println!("   | {}", linha.text);
        }
        if resultado.lines.len() > 6 {
            println!("   | ... mais {} linhas", resultado.lines.len() - 6);
        }
        println!();

        if args.desenhar {
            let saida = caminho.with_extension("boxes.png");
            desenhar(&imagem, &resultado, &saida)?;
            println!("   caixas desenhadas em {}\n", saida.display());
        }

        relatorio.push((nome, fria, quente, placar));
    }

    resumo(&relatorio);
    Ok(())
}

fn listar(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut saida = Vec::new();
    for entrada in std::fs::read_dir(dir)? {
        let caminho = entrada?.path();
        let ext = caminho
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let nome = caminho.to_string_lossy().to_lowercase();
        // `.boxes.png` sao saidas da propria ferramenta.
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg") && !nome.contains(".boxes.") {
            saida.push(caminho);
        }
    }
    saida.sort();
    Ok(saida)
}

struct Placar {
    recall: f64,
    acertos: usize,
    total: usize,
    faltando: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Gabarito {
    words: Vec<String>,
}

fn gabarito_de(imagem: &Path) -> Option<Vec<String>> {
    let caminho = imagem.with_extension("expected.json");
    let bruto = std::fs::read_to_string(caminho).ok()?;
    let gabarito: Gabarito = serde_json::from_str(&bruto).ok()?;
    Some(gabarito.words)
}

/// Recall por palavra: dos termos do gabarito, quantos o OCR leu em algum
/// lugar da tela.
///
/// Os dois lados passam pelo mesmo tokenizador. Sem isso, `Keyboard/Mouse`
/// lido pelo OCR jamais casaria com `Keyboard` no gabarito, e a nota puniria
/// uma leitura que na verdade estava certa.
fn avaliar(resultado: &OcrResult, esperadas: &[String]) -> Placar {
    let lidas: BTreeSet<String> = resultado
        .words
        .iter()
        .flat_map(|p| tokenizar(&p.text))
        .collect();

    let mut acertos = 0;
    let mut faltando = Vec::new();
    for esperada in esperadas {
        let alvo = tokenizar(esperada);
        if alvo.is_empty() {
            continue;
        }
        if alvo.iter().all(|t| lidas.contains(t)) {
            acertos += 1;
        } else {
            faltando.push(esperada.clone());
        }
    }

    let total = esperadas.len();
    Placar {
        recall: if total == 0 {
            1.0
        } else {
            acertos as f64 / total as f64
        },
        acertos,
        total,
        faltando,
    }
}

/// Quebra em palavras minusculas. Apostrofo conta como letra: `didn't` e uma
/// palavra so, nao duas.
fn tokenizar(texto: &str) -> Vec<String> {
    texto
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|t| !t.is_empty())
        .map(|t| t.trim_matches('\'').to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

fn desenhar(
    imagem: &RgbImage,
    resultado: &OcrResult,
    saida: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut copia = imagem.clone();
    // Linhas em vermelho, palavras em verde: da para ver de relance se o
    // agrupamento juntou o que devia e se a palavra caiu no lugar certo.
    for linha in &resultado.lines {
        retangulo(&mut copia, linha.bbox, Rgb([255, 0, 0]), 2);
    }
    for palavra in &resultado.words {
        retangulo(&mut copia, palavra.bbox, Rgb([0, 255, 0]), 1);
    }
    copia.save(saida)?;
    Ok(())
}

fn retangulo(imagem: &mut RgbImage, caixa: papaplay_lib::ocr::BBox, cor: Rgb<u8>, espessura: u32) {
    let (w, h) = imagem.dimensions();
    for t in 0..espessura {
        for x in caixa.x..caixa.right().min(w) {
            for y in [caixa.y + t, caixa.bottom().saturating_sub(1 + t)] {
                if y < h {
                    imagem.put_pixel(x, y, cor);
                }
            }
        }
        for y in caixa.y..caixa.bottom().min(h) {
            for x in [caixa.x + t, caixa.right().saturating_sub(1 + t)] {
                if x < w {
                    imagem.put_pixel(x, y, cor);
                }
            }
        }
    }
}

fn resumo(relatorio: &[(String, f64, f64, Option<Placar>)]) {
    println!("== resumo");
    println!(
        "{:<40} {:>10} {:>10} {:>10}",
        "imagem", "fria(ms)", "quente(ms)", "recall"
    );
    for (nome, fria, quente, placar) in relatorio {
        let recall = placar
            .as_ref()
            .map(|p| format!("{:.1}%", p.recall * 100.0))
            .unwrap_or_else(|| "-".into());
        let curto: String = nome.chars().take(40).collect();
        println!("{curto:<40} {fria:>10.0} {quente:>10.0} {recall:>10}");
    }

    let quentes: Vec<f64> = relatorio.iter().map(|(_, _, q, _)| *q).collect();
    if !quentes.is_empty() {
        let media = quentes.iter().sum::<f64>() / quentes.len() as f64;
        let pior = quentes.iter().copied().fold(f64::MIN, f64::max);
        println!(
            "\nlatencia quente: media {media:.0} ms, pior {pior:.0} ms (criterio GO: <500 ms)"
        );
    }

    let com_gabarito: Vec<&Placar> = relatorio
        .iter()
        .filter_map(|(_, _, _, p)| p.as_ref())
        .collect();
    if !com_gabarito.is_empty() {
        let acertos: usize = com_gabarito.iter().map(|p| p.acertos).sum();
        let total: usize = com_gabarito.iter().map(|p| p.total).sum();
        println!(
            "recall agregado: {:.1}% ({acertos}/{total}) em {} imagens com gabarito",
            acertos as f64 / total.max(1) as f64 * 100.0,
            com_gabarito.len()
        );
    }
}
