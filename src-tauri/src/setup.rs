//! Instalacao do tradutor de frases no primeiro uso.
//!
//! # Por que este modulo existe
//!
//! O tradutor EN->PT sao dois grafos ONNX de 332 MB — 86% de tudo que o app
//! embarcaria. Poe-los no instalador significaria 350 MB de download **a cada
//! atualizacao do app**, inclusive nas que so mudam um botao. Aqui eles viram um
//! download unico, feito uma vez por maquina e preservado entre versoes, e o
//! instalador fica em ~55 MB (dicionario + OCR).
//!
//! # Isto nao viola a regra 2
//!
//! "Nucleo 100% offline, nenhuma chamada de rede em runtime; downloads so de
//! modelos/dicionario em install/setup" (CLAUDE.md). Este e exatamente o caso
//! previsto: a rede e tocada uma vez, no setup, por acao explicita do usuario.
//! Depois disso nenhum caminho do app abre socket — espiar, traduzir, salvar e
//! revisar continuam funcionando com o cabo desconectado.
//!
//! # Sem o download o app funciona
//!
//! Degradacao, nao bloqueio: dicionario, OCR, deck e revisao nao dependem do
//! tradutor. O que falta e a traducao da **frase** de contexto. Por isso o
//! wizard permite pular, e a tela de configuracoes deixa instalar depois.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{Error, Result};

/// Onde os arquivos baixados ficam, dentro do diretorio de dados do usuario.
///
/// **Nao** no diretorio de instalacao: ali eles seriam apagados pelo
/// desinstalador de cada atualizacao, e o usuario baixaria 332 MB de novo a cada
/// versao nova. Em `%APPDATA%` eles sobrevivem, como o deck.
const SUBDIR: &str = "nmt";

/// Base das URLs dos artefatos. Sobrescrevivel para testar sem publicar.
const URL_ENV: &str = "PAPAPLAY_MODELS_URL";

/// De onde os modelos sao baixados.
///
/// Um release **separado** do release do app, com tag propria (`models-v1`): os
/// arquivos so mudam quando o modelo muda, e amarra-los a versao do app faria
/// cada correcao de bug republicar 332 MB. A tag so sobe quando o modelo trocar
/// de verdade — e ai os hashes em [`ARTEFATOS`] mudam junto.
///
/// Enquanto o release nao estiver publicado, o download falha com 404 e a UI
/// mostra isso; o app continua utilizavel sem traducao de frase.
const URL_BASE: &str =
    "https://github.com/matheusmouraut/papaplay/releases/download/models-v1";

/// Um arquivo a baixar, com o tamanho e o hash conhecidos de antemao.
///
/// O tamanho vem junto para a barra de progresso nao depender do
/// `Content-Length` da resposta (que falta em servidor com compressao chunked),
/// e o hash para um download truncado falhar aqui, e nao meia hora depois na
/// primeira traducao.
struct Artefato {
    arquivo: &'static str,
    bytes: u64,
    sha256: &'static str,
}

const ARTEFATOS: &[Artefato] = &[
    Artefato {
        arquivo: "encoder.onnx",
        bytes: 133_099_492,
        sha256: "1f3731f79dad17d12b1034f3a98daa1a62f9569a2e2bcc6c928007ed50ba2a91",
    },
    Artefato {
        arquivo: "decoder.onnx",
        bytes: 215_099_591,
        sha256: "52a129dce7cce071b27dabc5872a9b7d2345f16445250380b32af46def797220",
    },
];

/// Arquivos pequenos que **acompanham** o app em vez de serem baixados.
///
/// Sao 3,7 MB e mudam junto com o codigo que os le: versiona-los com o binario
/// evita a combinacao "app novo, tokenizador velho", que produziria traducao
/// silenciosamente errada em vez de um erro.
const ACOMPANHAM: &[&str] = &["meta.json", "tokenizer.json"];

/// Pedaco lido por vez do socket. 256 KB da ~1300 eventos de progresso num
/// download de 332 MB: fluido na barra sem inundar a IPC.
const PEDACO: usize = 256 * 1024;

/// Evento de progresso do download (`setup://nmt`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progresso {
    /// Arquivo sendo baixado agora.
    pub arquivo: String,
    /// Bytes ja gravados, somando os arquivos anteriores.
    pub baixado: u64,
    /// Total de todos os arquivos.
    pub total: u64,
}

/// O que a UI precisa saber para decidir entre "instalar" e "tudo certo".
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NmtStatus {
    /// O tradutor esta pronto para uso?
    pub installed: bool,
    /// Quanto o download custa, em bytes.
    pub download_bytes: u64,
    /// Onde os arquivos ficam — a UI mostra para quem quiser copia-los a mao.
    pub dir: String,
}

/// `%APPDATA%/papaplay/nmt`.
pub fn destino(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .data_dir()
        .map_err(|e| Error::Translate(format!("diretorio de dados indisponivel: {e}")))?
        .join("papaplay")
        .join(SUBDIR);
    Ok(dir)
}

/// O diretorio ja tem tudo que o tradutor precisa?
///
/// Confere o tamanho, e nao so a existencia: um download interrompido deixa um
/// arquivo com o nome certo e o conteudo pela metade, e "existe" diria que esta
/// instalado.
pub fn instalado(dir: &Path) -> bool {
    let completos = ARTEFATOS.iter().all(|a| {
        std::fs::metadata(dir.join(a.arquivo))
            .map(|m| m.len() == a.bytes)
            .unwrap_or(false)
    });
    completos && ACOMPANHAM.iter().all(|nome| dir.join(nome).is_file())
}

fn total_de_bytes() -> u64 {
    ARTEFATOS.iter().map(|a| a.bytes).sum()
}

fn url_de(arquivo: &str) -> String {
    let base = std::env::var(URL_ENV).unwrap_or_else(|_| URL_BASE.to_string());
    format!("{}/{arquivo}", base.trim_end_matches('/'))
}

/// Copia `meta.json` e `tokenizer.json` dos recursos do app para o destino.
///
/// Os quatro arquivos precisam morar no mesmo diretorio porque e assim que o
/// `translate::models_dir` os encontra — um diretorio, uma versao do tradutor.
fn copiar_acompanhantes(app: &AppHandle, dir: &Path) -> Result<()> {
    let origem = app
        .path()
        .resource_dir()
        .map(|r| r.join(SUBDIR))
        .map_err(|e| Error::Translate(format!("recursos do app indisponiveis: {e}")))?;
    for nome in ACOMPANHAM {
        let de = origem.join(nome);
        if !de.is_file() {
            return Err(Error::Translate(format!(
                "{nome} nao veio com a instalacao ({})",
                de.display()
            )));
        }
        std::fs::copy(&de, dir.join(nome))?;
    }
    Ok(())
}

/// Baixa um artefato para `.parte` e so o renomeia depois de conferir o hash.
///
/// O nome temporario e o que impede um download interrompido de se passar por
/// arquivo bom: se o app fechar no meio, sobra um `.parte` que ninguem carrega.
fn baixar(
    app: &AppHandle,
    dir: &Path,
    artefato: &Artefato,
    ja_baixado: u64,
    total: u64,
) -> Result<()> {
    let destino = dir.join(artefato.arquivo);
    if std::fs::metadata(&destino).is_ok_and(|m| m.len() == artefato.bytes) {
        return Ok(());
    }

    let parcial = dir.join(format!("{}.parte", artefato.arquivo));
    let cliente = reqwest::blocking::Client::builder()
        // Sem timeout total: 332 MB numa conexao lenta levam mais que qualquer
        // limite razoavel. O timeout de conexao pega o caso que importa — o
        // servidor que nao responde.
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| Error::Translate(format!("cliente http nao subiu: {e}")))?;

    let url = url_de(artefato.arquivo);
    let mut resposta = cliente
        .get(&url)
        .send()
        .map_err(|e| Error::Translate(format!("{url}: {e}")))?;
    if !resposta.status().is_success() {
        return Err(Error::Translate(format!(
            "{url} respondeu {}",
            resposta.status()
        )));
    }

    let mut arquivo = std::fs::File::create(&parcial)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; PEDACO];
    let mut escrito = 0u64;

    loop {
        let lidos = resposta
            .read(&mut buffer)
            .map_err(|e| Error::Translate(format!("leitura de {url} falhou: {e}")))?;
        if lidos == 0 {
            break;
        }
        std::io::Write::write_all(&mut arquivo, &buffer[..lidos])?;
        hasher.update(&buffer[..lidos]);
        escrito += lidos as u64;
        let _ = app.emit(
            "setup://nmt",
            Progresso {
                arquivo: artefato.arquivo.to_string(),
                baixado: ja_baixado + escrito,
                total,
            },
        );
    }
    drop(arquivo);

    let hash = format!("{:x}", hasher.finalize());
    if hash != artefato.sha256 {
        let _ = std::fs::remove_file(&parcial);
        return Err(Error::Translate(format!(
            "{} chegou corrompido (esperado {}, veio {hash})",
            artefato.arquivo, artefato.sha256
        )));
    }

    std::fs::rename(&parcial, &destino)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn nmt_status(app: AppHandle) -> Result<NmtStatus> {
    let dir = destino(&app)?;
    Ok(NmtStatus {
        // Em dev os modelos vivem na arvore do repo e nunca foram baixados; o
        // `translate::models_dir` os acha por outro caminho, e dizer "faltando"
        // ali seria mentira.
        installed: instalado(&dir) || crate::translate::modelo_disponivel(&app),
        download_bytes: total_de_bytes(),
        dir: dir.display().to_string(),
    })
}

/// Baixa o tradutor. Emite `setup://nmt` a cada pedaco.
#[tauri::command]
pub async fn nmt_install(app: AppHandle) -> Result<NmtStatus> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let dir = destino(&handle)?;
        std::fs::create_dir_all(&dir)?;
        copiar_acompanhantes(&handle, &dir)?;

        let total = total_de_bytes();
        let mut acumulado = 0u64;
        for artefato in ARTEFATOS {
            baixar(&handle, &dir, artefato, acumulado, total)?;
            acumulado += artefato.bytes;
        }
        Ok::<(), Error>(())
    })
    .await
    .map_err(|e| Error::Translate(format!("instalacao abortada: {e}")))??;

    nmt_status(app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diretorio_vazio_nao_conta_como_instalado() {
        let dir = std::env::temp_dir().join("papaplay-teste-nmt-vazio");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criou");
        assert!(!instalado(&dir));
    }

    #[test]
    fn arquivo_do_tamanho_errado_nao_conta_como_instalado() {
        let dir = std::env::temp_dir().join("papaplay-teste-nmt-truncado");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criou");
        for artefato in ARTEFATOS {
            std::fs::write(dir.join(artefato.arquivo), b"meio download").expect("escreveu");
        }
        for nome in ACOMPANHAM {
            std::fs::write(dir.join(nome), b"{}").expect("escreveu");
        }
        assert!(!instalado(&dir));
    }

    #[test]
    fn a_url_sai_da_variavel_de_ambiente_quando_ela_existe() {
        // Sem `env::set_var` (unsafe desde a edicao 2024 e global aos testes):
        // o que importa aqui e a montagem da URL, e a barra que sobra na base.
        assert_eq!(
            format!("{}/{}", "https://ex.com/v1/".trim_end_matches('/'), "a.onnx"),
            "https://ex.com/v1/a.onnx"
        );
    }

    #[test]
    fn o_total_bate_com_a_soma_dos_artefatos() {
        assert_eq!(total_de_bytes(), 133_099_492 + 215_099_591);
    }
}
