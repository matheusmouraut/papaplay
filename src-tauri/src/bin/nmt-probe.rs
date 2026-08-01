//! Prova de fogo do tradutor: roda o `translate::Engine` sobre as mesmas frases
//! que `scripts/export-nmt.py` usa para validar o export.
//!
//! Existe porque os dois lados podem divergir em silencio: o Python valida o
//! ONNX contra o PyTorch, mas quem decodifica em producao e o Rust. Se uma
//! frase sai diferente aqui, o erro esta na decodificacao deste repo, nao no
//! modelo.
//!
//! ```powershell
//! cargo run --release --bin nmt-probe
//! cargo run --release --bin nmt-probe -- --frase "Watch out!" --repeticoes 3
//! ```

use std::path::PathBuf;
use std::time::Instant;

use papaplay_lib::translate::Engine;

/// As mesmas de `FRASES_DE_PROVA` em `scripts/export-nmt.py` — fala de jogo,
/// imperativo, segunda pessoa, nome proprio e pontuacao colada.
const FRASES: &[&str] = &[
    "The dread of the deep keeps most sailors ashore.",
    "You must gather your party before venturing forth.",
    "I used to be an adventurer like you, then I took an arrow in the knee.",
    "Press the button to open the gate.",
    "Your journey ends here, wanderer.",
    "The merchant refuses to trade with you until you pay your debt.",
    "A strange light flickers deep within the cave.",
    "She gave up on finding the missing shipment.",
    "Watch out! The bridge is collapsing!",
    "This sword deals extra damage to undead creatures.",
    "Talk to the innkeeper if you need a place to rest.",
    "He was told the war had ended years ago.",
];

fn main() {
    let mut frase: Option<String> = None;
    let mut repeticoes = 1usize;
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--frase" => frase = iter.next(),
            "--repeticoes" => {
                repeticoes = iter.next().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
            }
            outro => eprintln!("argumento ignorado: {outro}"),
        }
    }

    let dir = match diretorio() {
        Some(dir) => dir,
        None => {
            eprintln!(
                "modelo nao encontrado. Rode `powershell -File scripts/export-nmt.ps1` \
                 ou aponte PAPAPLAY_NMT_MODELS para o diretorio dele."
            );
            std::process::exit(1);
        }
    };
    println!("modelo: {}", dir.display());

    let comeco = Instant::now();
    let mut engine = match Engine::load(&dir) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("falha ao carregar: {e}");
            std::process::exit(1);
        }
    };
    println!("carga:  {} ms\n", comeco.elapsed().as_millis());

    let frases: Vec<&str> = match &frase {
        Some(uma) => vec![uma.as_str()],
        None => FRASES.to_vec(),
    };

    let mut tempos = Vec::new();
    for texto in frases {
        for i in 0..repeticoes {
            let comeco = Instant::now();
            let saida = engine.translate(texto);
            let ms = comeco.elapsed().as_secs_f64() * 1000.0;
            match saida {
                Ok(pt) => {
                    // Repeticao existe para medir, nao para reimprimir: so a
                    // primeira volta mostra o texto.
                    if i == 0 {
                        println!("  en: {texto}");
                        println!("  pt: {pt}");
                    }
                    println!("      {ms:6.0} ms");
                    tempos.push(ms);
                }
                Err(e) => println!("  ERRO em {texto:?}: {e}"),
            }
        }
        println!();
    }

    if tempos.is_empty() {
        std::process::exit(1);
    }
    tempos.sort_by(|a, b| a.total_cmp(b));
    let soma: f64 = tempos.iter().sum();
    println!(
        "{} traducoes | media {:.0} ms | mediana {:.0} ms | pior {:.0} ms",
        tempos.len(),
        soma / tempos.len() as f64,
        tempos[tempos.len() / 2],
        tempos[tempos.len() - 1],
    );
}

/// Mesma ordem de [`papaplay_lib::translate`], sem o `AppHandle`: variavel de
/// ambiente e depois a arvore do repo.
fn diretorio() -> Option<PathBuf> {
    if let Some(bruto) = std::env::var_os("PAPAPLAY_NMT_MODELS") {
        return Some(PathBuf::from(bruto));
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/nmt");
    repo.join("meta.json").is_file().then_some(repo)
}
