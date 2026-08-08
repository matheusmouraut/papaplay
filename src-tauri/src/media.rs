//! Screenshots dos contextos: o recorte da frase, em `.webp`, na pasta `media/`.
//!
//! # Por que arquivo e nao blob
//!
//! O banco do usuario e insubstituivel e precisa caber num backup rapido
//! (docs/04). Guardar imagem dentro dele faria o `papaplay.db` crescer centenas
//! de megabytes em meses de uso; em `media/` o banco guarda so o caminho e a
//! pasta pode ser sincronizada, podada ou perdida sem levar o deck junto.
//!
//! # Por que recortar
//!
//! O card quer a **frase**, nao a tela. Um recorte de uma linha de dialogo tem
//! alguns kilobytes e ainda mostra a fonte e o cenario onde a palavra apareceu —
//! que e o gancho de memoria que faz a revisao funcionar (F4). Guardar o frame
//! inteiro seria 100x maior sem ensinar nada a mais.
//!
//! # Por que lossless
//!
//! O encoder de webp do `image` e Rust puro e so faz lossless. Para um recorte
//! de texto isso e uma boa troca: texto tem poucas cores e comprime muito bem
//! sem artefato, e o alternativo seria compilar o libwebp (C) por causa de uma
//! imagem de 30 KB.

use std::path::{Path, PathBuf};

use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};
use tauri::AppHandle;

use crate::db;
use crate::error::{Error, Result};
use crate::ocr::BBox;

/// Nome da pasta, ao lado do `papaplay.db`. Tambem e o prefixo do caminho
/// relativo gravado em `contexts.screenshot_path`.
pub const PASTA: &str = "media";

/// Folga em volta da linha, em pixels fisicos: `(horizontal, vertical)`.
///
/// Assimetrica de proposito. Na horizontal ela mostra que a frase continua (ou
/// que acabou), o que ajuda a reconhecer a cena; na vertical, folga demais
/// puxaria a linha de dialogo de cima para dentro do recorte.
pub const MARGEM: (u32, u32) = (24, 12);

/// Imagem BGRA em memoria, como a captura entrega.
pub struct Bgra<'a> {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: &'a [u8],
}

/// Recorte pronto para virar arquivo. Ja em RGBA, que e o que o encoder fala.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorte {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Recorta `area` (mais `margem`) da imagem, convertendo BGRA -> RGBA.
///
/// Devolve `None` quando nao sobra nada para recortar: area fora do frame,
/// frame vazio ou buffer menor do que as dimensoes prometem. Nenhum desses
/// casos merece derrubar o salvamento do card — o card so fica sem imagem.
pub fn recortar(fonte: &Bgra<'_>, area: BBox, margem: (u32, u32)) -> Option<Recorte> {
    let esperado = (fonte.width as usize).saturating_mul(fonte.height as usize) * 4;
    if esperado == 0 || fonte.pixels.len() < esperado {
        return None;
    }

    let x0 = area.x.saturating_sub(margem.0);
    let y0 = area.y.saturating_sub(margem.1);
    let x1 = area.right().saturating_add(margem.0).min(fonte.width);
    let y1 = area.bottom().saturating_add(margem.1).min(fonte.height);
    // Cobre tambem a area que comeca depois da borda: ali `x1` (limitado pela
    // largura) fica antes de `x0`.
    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    let largura = x1 - x0;
    let altura = y1 - y0;
    let mut pixels = Vec::with_capacity(largura as usize * altura as usize * 4);
    for y in y0..y1 {
        let inicio = (y as usize * fonte.width as usize + x0 as usize) * 4;
        let fim = inicio + largura as usize * 4;
        for px in fonte.pixels[inicio..fim].chunks_exact(4) {
            // Alfa fixado em opaco: o WGC devolve 0 no canal alfa de conteudo
            // opaco, e respeitar isso daria um screenshot invisivel.
            pixels.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }

    Some(Recorte {
        width: largura,
        height: altura,
        pixels,
    })
}

/// Pasta dos screenshots — ao lado do banco, inclusive quando `PAPAPLAY_DB`
/// aponta para outro lugar: deck e midia andam juntos ou o card perde a imagem.
pub fn dir(app: &AppHandle) -> Result<PathBuf> {
    let banco = db::caminho(app)?;
    let pai = banco
        .parent()
        .ok_or_else(|| Error::Media(format!("banco sem diretorio: {}", banco.display())))?;
    Ok(pai.join(PASTA))
}

/// Grava o recorte como `<dir>/<nome>.webp` e devolve o caminho **relativo**
/// (`media/<nome>.webp`), que e o que vai para `contexts.screenshot_path`.
///
/// Relativo porque o caminho absoluto muda de maquina e de usuario: um deck
/// restaurado noutro PC continua achando as imagens.
pub fn salvar(app: &AppHandle, nome: &str, recorte: &Recorte) -> Result<String> {
    let dir = dir(app)?;
    std::fs::create_dir_all(&dir)?;
    gravar(&dir.join(format!("{nome}.webp")), recorte)?;
    Ok(format!("{PASTA}/{nome}.webp"))
}

/// Resolve um caminho relativo do banco (`media/ctx-000012.webp`) em caminho
/// absoluto, recusando qualquer coisa que aponte para fora da pasta.
///
/// O valor vem do banco, nao da rede — mas e o banco que sobrevive a
/// importacoes e a edicoes manuais, e um `..` ali viraria leitura (ou remocao)
/// de arquivo arbitrario a partir de um comando exposto a UI.
pub fn resolver(app: &AppHandle, relativo: &str) -> Result<PathBuf> {
    Ok(dir(app)?.join(nome_de_arquivo(relativo)?))
}

/// Extrai o nome do arquivo de um caminho relativo do banco.
///
/// Aceita so um componente simples depois do prefixo `media/`: nada de `..`,
/// nada de subpasta, nada de caminho absoluto.
fn nome_de_arquivo(relativo: &str) -> Result<&Path> {
    let dentro_da_pasta = relativo
        .strip_prefix(&format!("{PASTA}/"))
        .or_else(|| relativo.strip_prefix(&format!("{PASTA}\\")))
        .unwrap_or(relativo);
    let nome = Path::new(dentro_da_pasta);

    let mut componentes = nome.components();
    match (componentes.next(), componentes.next()) {
        (Some(std::path::Component::Normal(_)), None) => Ok(nome),
        _ => Err(Error::Media(format!("caminho suspeito: {relativo}"))),
    }
}

/// Bytes do screenshot, para a UI mostrar a imagem.
pub fn ler(app: &AppHandle, relativo: &str) -> Result<Vec<u8>> {
    let caminho = resolver(app, relativo)?;
    std::fs::read(&caminho).map_err(|e| Error::Media(format!("nao leu {}: {e}", caminho.display())))
}

/// Apaga o arquivo de um contexto que deixou de existir. Arquivo ausente nao e
/// erro: o resultado pedido — nao existir — ja e o que se tem.
pub fn remover(app: &AppHandle, relativo: &str) -> Result<()> {
    let caminho = resolver(app, relativo)?;
    match std::fs::remove_file(&caminho) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Media(format!(
            "nao apagou {}: {e}",
            caminho.display()
        ))),
    }
}

/// Codifica e escreve o arquivo. Separado de [`salvar`] porque e a parte que os
/// testes conseguem exercitar sem um `AppHandle`.
pub fn gravar(caminho: &Path, recorte: &Recorte) -> Result<()> {
    let arquivo = std::fs::File::create(caminho)?;
    WebPEncoder::new_lossless(std::io::BufWriter::new(arquivo))
        .write_image(
            &recorte.pixels,
            recorte.width,
            recorte.height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| Error::Media(format!("nao gravou {}: {e}", caminho.display())))
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

/// Bytes do screenshot de um contexto, para a `<img>` da tela Deck.
///
/// Vai pela IPC em vez do protocolo `asset://` de proposito: o caminho do banco
/// e configuravel (`PAPAPLAY_DB`) e um escopo estatico no `tauri.conf.json` nao
/// acompanharia isso — aqui quem valida o caminho e [`resolver`].
#[tauri::command]
pub async fn media_screenshot(app: AppHandle, path: String) -> Result<tauri::ipc::Response> {
    tauri::async_runtime::spawn_blocking(move || ler(&app, &path).map(tauri::ipc::Response::new))
        .await
        .map_err(|e| Error::Media(format!("leitura do screenshot abortada: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Imagem BGRA em que cada pixel guarda a propria posicao: `B` = x, `G` = y.
    /// Da para conferir de onde o recorte veio olhando um pixel so.
    fn imagem(largura: u32, altura: u32) -> Vec<u8> {
        let mut pixels = Vec::with_capacity((largura * altura * 4) as usize);
        for y in 0..altura {
            for x in 0..largura {
                pixels.extend_from_slice(&[x as u8, y as u8, 0, 0]);
            }
        }
        pixels
    }

    fn fonte(largura: u32, altura: u32, pixels: &[u8]) -> Bgra<'_> {
        Bgra {
            width: largura,
            height: altura,
            pixels,
        }
    }

    fn bbox(x: u32, y: u32, w: u32, h: u32) -> BBox {
        BBox { x, y, w, h }
    }

    #[test]
    fn a_margem_entra_dos_dois_lados() {
        let pixels = imagem(100, 100);
        let r =
            recortar(&fonte(100, 100, &pixels), bbox(40, 40, 20, 10), (5, 2)).expect("recortou");
        assert_eq!((r.width, r.height), (30, 14));
        assert_eq!(r.pixels.len(), 30 * 14 * 4);
    }

    #[test]
    fn perto_da_borda_a_margem_e_aparada_sem_erro() {
        // A frase costuma ficar colada na borda de baixo da tela: se o recorte
        // falhasse ali, o card do dialogo — o caso comum — ficaria sem imagem.
        let pixels = imagem(100, 100);
        let r =
            recortar(&fonte(100, 100, &pixels), bbox(0, 90, 100, 10), (24, 12)).expect("recortou");
        assert_eq!((r.width, r.height), (100, 22), "aparou nas quatro bordas");
    }

    #[test]
    fn bgra_vira_rgba_com_alfa_opaco() {
        let pixels = imagem(10, 10);
        let r = recortar(&fonte(10, 10, &pixels), bbox(3, 4, 1, 1), (0, 0)).expect("recortou");
        // O pixel (3,4) foi gravado como B=3, G=4, R=0, A=0.
        assert_eq!(r.pixels, vec![0, 4, 3, 255], "R, G, B, A nesta ordem");
    }

    #[test]
    fn area_fora_do_frame_nao_recorta() {
        let pixels = imagem(50, 50);
        assert!(recortar(&fonte(50, 50, &pixels), bbox(80, 80, 10, 10), (0, 0)).is_none());
    }

    #[test]
    fn buffer_menor_que_as_dimensoes_nao_recorta() {
        // Defesa contra um frame truncado: melhor card sem imagem do que panico
        // de indice no meio do salvamento.
        let pixels = imagem(10, 10);
        assert!(recortar(&fonte(20, 20, &pixels), bbox(0, 0, 5, 5), (0, 0)).is_none());
    }

    #[test]
    fn frame_vazio_nao_recorta() {
        assert!(recortar(&fonte(0, 0, &[]), bbox(0, 0, 5, 5), (0, 0)).is_none());
    }

    #[test]
    fn o_caminho_do_banco_vira_o_nome_do_arquivo() {
        assert_eq!(
            nome_de_arquivo("media/ctx-000012.webp").expect("aceito"),
            Path::new("ctx-000012.webp")
        );
        // Sem o prefixo tambem vale: e assim que um caminho ja resolvido volta
        // para ca sem virar erro.
        assert_eq!(
            nome_de_arquivo("ctx-000012.webp").expect("aceito"),
            Path::new("ctx-000012.webp")
        );
    }

    #[test]
    fn caminho_que_sai_da_pasta_e_recusado() {
        // `ler` e `remover` sao comandos expostos a UI: um `..` vindo de um
        // banco importado nao pode virar leitura (nem remocao) fora de media/.
        for suspeito in [
            "media/../../segredo.txt",
            "../segredo.txt",
            "sub/pasta.webp",
            "C:\\Windows\\system.ini",
            "/etc/passwd",
            "",
        ] {
            assert!(
                nome_de_arquivo(suspeito).is_err(),
                "deveria recusar {suspeito:?}"
            );
        }
    }

    #[test]
    fn o_arquivo_gravado_e_um_webp_legivel() {
        let pixels = imagem(64, 32);
        let recorte =
            recortar(&fonte(64, 32, &pixels), bbox(8, 8, 32, 8), MARGEM).expect("recortou");

        let caminho =
            std::env::temp_dir().join(format!("papaplay-media-{}.webp", std::process::id()));
        let _ = std::fs::remove_file(&caminho);
        gravar(&caminho, &recorte).expect("gravou");

        let lido = image::open(&caminho).expect("webp valido");
        assert_eq!(lido.width(), recorte.width);
        assert_eq!(lido.height(), recorte.height);

        let _ = std::fs::remove_file(&caminho);
    }
}
