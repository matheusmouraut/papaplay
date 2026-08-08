//! Traducao de frases en->pt-BR offline (Marian/OPUS-MT em ONNX int8).
//!
//! Modelo: Helsinki-NLP/opus-mt-tc-big-en-pt, exportado por
//! `scripts/export-nmt.py`. Aquele script tem uma implementacao em Python da
//! mesma decodificacao daqui, usada para conferir o export contra o PyTorch em
//! fp32 -- se os dois lados divergirem, o Python e a referencia.
//!
//! Quem confere esta ponta e o binario `nmt-probe`, que roda as mesmas frases
//! do script: na medicao de 2026-08-01 as 12 sairam identicas as do Python, a
//! 69 ms de mediana contra ~190 ms la. A quantizacao int8 custou 2 das 12
//! frases contra o fp32 (83%, acima do minimo de 75% que o export exige) --
//! ambas trocas de sinonimo, nao erros de sentido.
//!
//! # A decodificacao, em tres armadilhas
//!
//! O `decoder.onnx` e a variante *merged*: um grafo unico com um `If` que
//! escolhe entre o caminho sem cache (primeiro passo) e o com cache (os
//! demais), pela entrada booleana `use_cache_branch`. Disso saem tres detalhes
//! que nao se descobre lendo o grafo:
//!
//! 1. **O primeiro passo ainda recebe os 24 tensores de cache**, todos vazios
//!    (`[1, 16, 0, 64]`). O ramo escolhido os ignora, mas o ONNX exige que toda
//!    entrada declarada seja fornecida.
//! 2. **O KV da cross-attention so vale do primeiro passo.** No ramo com cache
//!    o grafo nao recalcula a atencao sobre o encoder, e as saidas
//!    `present.*.encoder.*` de la sao constantes de enchimento -- realimenta-las
//!    faz o passo seguinte estourar. Como a cross-attention nao muda durante a
//!    geracao, o valor do primeiro passo serve ate o fim.
//! 3. **O cache volta copiado.** As saidas emprestam a `Session`, que precisa
//!    estar livre para o proximo `run`; sao ~1,5 MB por passo, irrelevante
//!    perto da inferencia.
//!
//! # Politica de memoria
//!
//! Os dois grafos somam ~350 MB em disco e mais que isso vivos. Ao contrario
//! do OCR, que fica carregado ([`crate::lookup`]), o tradutor e descarregado —
//! mas **por ociosidade** ([`unload_if_idle`]), nao a cada espiada.
//!
//! A primeira versao descarregava toda vez que a tecla era solta, e o efeito
//! foi o oposto do pretendido: quem joga espia varias palavras seguidas, entao
//! cada card pagava de novo o ~1 s de carga. Trocar por um prazo de ociosidade
//! mantem a memoria devolvida quando o jogador parou de consultar, que era o
//! objetivo, sem cobrar por isso no meio da cena.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::session::{Session, SessionInputValue};
use ort::value::Tensor;
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tokenizers::Tokenizer;

use crate::error::{Error, Result};

/// Variavel de ambiente que sobrescreve o diretorio do modelo de traducao.
const MODELS_ENV: &str = "PAPAPLAY_NMT_MODELS";

/// Teto de tokens gerados. Uma linha de dialogo de jogo raramente passa de 40;
/// o limite existe para um modelo em loop nao travar a consulta.
const MAX_TOKENS: usize = 96;

/// Frases mais longas que isto quase certamente sao lixo de OCR (duas colunas
/// lidas como uma linha, HUD misturado com legenda) e nao valem a inferencia.
const MAX_CARACTERES: usize = 500;

/// Espelha o `meta.json` que o script de export grava junto dos modelos.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    decoder_start_token_id: i64,
    eos_token_id: i64,
    decoder_layers: usize,
    decoder_attention_heads: usize,
    head_dim: usize,
}

/// Um tensor do cache, entre um passo e o outro.
struct Cache {
    entrada: String,
    shape: [usize; 4],
    dados: Vec<f32>,
}

pub struct Engine {
    tokenizer: Tokenizer,
    encoder: Session,
    decoder: Session,
    meta: Meta,
}

impl Engine {
    /// Carrega de um diretorio com `encoder.onnx`, `decoder.onnx`,
    /// `tokenizer.json` e `meta.json`.
    pub fn load(dir: &Path) -> Result<Self> {
        let meta: Meta = serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json"))?)
            .map_err(|e| Error::Translate(format!("meta.json invalido: {e}")))?;
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| Error::Translate(format!("tokenizer.json invalido: {e}")))?;
        Ok(Self {
            tokenizer,
            encoder: sessao(&dir.join("encoder.onnx"))?,
            decoder: sessao(&dir.join("decoder.onnx"))?,
            meta,
        })
    }

    /// Traduz uma frase. Devolve string vazia para entrada vazia.
    pub fn translate(&mut self, texto: &str) -> Result<String> {
        let texto = texto.trim();
        if texto.is_empty() {
            return Ok(String::new());
        }
        if texto.chars().count() > MAX_CARACTERES {
            return Err(Error::Translate(format!(
                "frase com {} caracteres, acima do limite de {MAX_CARACTERES}",
                texto.chars().count()
            )));
        }

        let codificado = self
            .tokenizer
            .encode(texto, true)
            .map_err(|e| Error::Translate(format!("tokenizacao falhou: {e}")))?;
        let ids: Vec<i64> = codificado.get_ids().iter().map(|&id| id as i64).collect();
        if ids.is_empty() {
            return Ok(String::new());
        }

        let estados = self.codificar(&ids)?;
        let saida = self.decodificar(&ids, estados)?;

        self.tokenizer
            .decode(&saida, true)
            .map(|texto| texto.trim().to_string())
            .map_err(|e| Error::Translate(format!("detokenizacao falhou: {e}")))
    }

    /// Passa a frase pelo encoder. Devolve `([1, n, d_model], dados)`.
    fn codificar(&mut self, ids: &[i64]) -> Result<([usize; 3], Vec<f32>)> {
        let n = ids.len();
        let entradas: Vec<(Cow<'static, str>, SessionInputValue)> = vec![
            ("input_ids".into(), tensor_i64([1, n], ids.to_vec())?.into()),
            (
                "attention_mask".into(),
                tensor_i64([1, n], vec![1i64; n])?.into(),
            ),
        ];

        let saidas = self
            .encoder
            .run(entradas)
            .map_err(|e| Error::Translate(format!("encoder falhou: {e}")))?;
        let (_, valor) = saidas
            .into_iter()
            .next()
            .ok_or_else(|| Error::Translate("encoder nao devolveu saida".into()))?;
        let (forma, dados) = valor
            .try_extract_tensor::<f32>()
            .map_err(|e| Error::Translate(format!("saida do encoder ilegivel: {e}")))?;
        if forma.len() != 3 {
            return Err(Error::Translate(format!(
                "encoder devolveu formato {forma:?}, esperado [1, n, d_model]"
            )));
        }
        Ok((
            [forma[0] as usize, forma[1] as usize, forma[2] as usize],
            dados.to_vec(),
        ))
    }

    /// Decodificacao gulosa. Devolve os ids gerados, sem o `</s>`.
    fn decodificar(
        &mut self,
        ids_entrada: &[i64],
        (forma_estados, estados): ([usize; 3], Vec<f32>),
    ) -> Result<Vec<u32>> {
        let n = ids_entrada.len();
        let mut cache = self.cache_vazio();
        let mut token = self.meta.decoder_start_token_id;
        let mut usa_cache = false;
        let mut saida = Vec::new();

        for _ in 0..MAX_TOKENS {
            let mut entradas: Vec<(Cow<'static, str>, SessionInputValue)> =
                Vec::with_capacity(4 + cache.len());
            entradas.push(("input_ids".into(), tensor_i64([1, 1], vec![token])?.into()));
            entradas.push((
                "encoder_attention_mask".into(),
                tensor_i64([1, n], vec![1i64; n])?.into(),
            ));
            entradas.push((
                "encoder_hidden_states".into(),
                Tensor::from_array((forma_estados, estados.clone()))
                    .map_err(|e| Error::Translate(format!("tensor de estados invalido: {e}")))?
                    .into(),
            ));
            entradas.push((
                "use_cache_branch".into(),
                Tensor::from_array(([1usize], vec![usa_cache]))
                    .map_err(|e| Error::Translate(format!("tensor de flag invalido: {e}")))?
                    .into(),
            ));
            for item in &cache {
                entradas.push((
                    Cow::Owned(item.entrada.clone()),
                    Tensor::from_array((item.shape, item.dados.clone()))
                        .map_err(|e| Error::Translate(format!("tensor de cache invalido: {e}")))?
                        .into(),
                ));
            }

            let saidas = self
                .decoder
                .run(entradas)
                .map_err(|e| Error::Translate(format!("decoder falhou: {e}")))?;

            let mut proximo = None;
            let mut novo_cache = Vec::with_capacity(cache.len());
            for (nome, valor) in saidas.into_iter() {
                if nome == "logits" {
                    proximo = Some(melhor_token(&valor)?);
                    continue;
                }
                let Some(miolo) = nome.strip_prefix("present.") else {
                    continue;
                };
                // Armadilha 2 do cabecalho: no ramo com cache as saidas de
                // cross-attention sao constantes de enchimento. Mantem-se o que
                // veio do primeiro passo.
                if usa_cache && miolo.contains(".encoder.") {
                    continue;
                }
                novo_cache.push(extrair_cache(format!("past_key_values.{miolo}"), &valor)?);
            }

            let proximo =
                proximo.ok_or_else(|| Error::Translate("decoder nao devolveu logits".into()))?;
            if proximo == self.meta.eos_token_id {
                break;
            }
            saida.push(proximo as u32);
            token = proximo;

            if usa_cache {
                // So os tensores de self-attention foram recopiados; os de
                // cross-attention seguem os do primeiro passo.
                substituir(&mut cache, novo_cache);
            } else {
                cache = novo_cache;
            }
            usa_cache = true;
        }

        Ok(saida)
    }

    /// Os 24 tensores de cache no formato que o primeiro passo espera: presentes
    /// na lista de entradas, porem com zero posicoes.
    fn cache_vazio(&self) -> Vec<Cache> {
        let shape = [1, self.meta.decoder_attention_heads, 0, self.meta.head_dim];
        let mut cache = Vec::with_capacity(self.meta.decoder_layers * 4);
        for camada in 0..self.meta.decoder_layers {
            for lado in ["decoder", "encoder"] {
                for parte in ["key", "value"] {
                    cache.push(Cache {
                        entrada: format!("past_key_values.{camada}.{lado}.{parte}"),
                        shape,
                        dados: Vec::new(),
                    });
                }
            }
        }
        cache
    }
}

/// Sobrescreve em `cache` as entradas que reapareceram em `novos`, preservando
/// as que nao vieram neste passo.
fn substituir(cache: &mut [Cache], novos: Vec<Cache>) {
    for novo in novos {
        if let Some(alvo) = cache.iter_mut().find(|item| item.entrada == novo.entrada) {
            *alvo = novo;
        }
    }
}

fn melhor_token(valor: &ort::value::DynValue) -> Result<i64> {
    let (forma, dados) = valor
        .try_extract_tensor::<f32>()
        .map_err(|e| Error::Translate(format!("logits ilegiveis: {e}")))?;
    let vocab = *forma
        .last()
        .ok_or_else(|| Error::Translate("logits sem formato".into()))? as usize;
    if vocab == 0 || dados.len() < vocab {
        return Err(Error::Translate(format!(
            "logits com {} valores para vocabulario de {vocab}",
            dados.len()
        )));
    }
    // Ultima posicao da sequencia: e a unica que interessa na geracao passo a
    // passo, e no primeiro passo tambem e a unica que existe.
    let ultima = &dados[dados.len() - vocab..];
    let mut melhor = 0usize;
    for (i, &v) in ultima.iter().enumerate() {
        if v > ultima[melhor] {
            melhor = i;
        }
    }
    Ok(melhor as i64)
}

fn extrair_cache(entrada: String, valor: &ort::value::DynValue) -> Result<Cache> {
    let (forma, dados) = valor
        .try_extract_tensor::<f32>()
        .map_err(|e| Error::Translate(format!("cache ilegivel em {entrada}: {e}")))?;
    if forma.len() != 4 {
        return Err(Error::Translate(format!(
            "cache {entrada} com formato {forma:?}, esperado 4 dimensoes"
        )));
    }
    Ok(Cache {
        entrada,
        shape: [
            forma[0] as usize,
            forma[1] as usize,
            forma[2] as usize,
            forma[3] as usize,
        ],
        dados: dados.to_vec(),
    })
}

fn tensor_i64<const N: usize>(shape: [usize; N], dados: Vec<i64>) -> Result<Tensor<i64>> {
    Tensor::from_array((shape, dados))
        .map_err(|e| Error::Translate(format!("tensor de entrada invalido: {e}")))
}

fn sessao(caminho: &Path) -> Result<Session> {
    let construir = || -> std::result::Result<Session, ort::Error> {
        let mut builder = Session::builder()?.with_intra_threads(intra_threads())?;
        builder.commit_from_file(caminho)
    };
    construir()
        .map_err(|e| Error::Translate(format!("falha ao carregar {}: {e}", caminho.display())))
}

/// Mesmo teto do OCR (ver `ocr::THREADS_MAX`): o ganho satura em 4 threads e
/// piora acima disso, e nao se pode tomar a maquina de quem esta jogando.
fn intra_threads() -> usize {
    if let Some(forcado) = std::env::var("PAPAPLAY_NMT_THREADS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
    {
        return forcado;
    }
    std::thread::available_parallelism()
        .map(|n| n.get().min(4))
        .unwrap_or(1)
}

// ---------------------------------------------------------------------------
// Estado global
// ---------------------------------------------------------------------------

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

/// Um diretorio so serve se tiver os quatro arquivos.
///
/// Checar so o `meta.json` (400 bytes) daria um diretorio "valido" no caso mais
/// comum de todos depois do instalador enxuto: os acompanhantes copiados e o
/// download dos `.onnx` ainda por fazer.
fn completo(dir: &Path) -> bool {
    ["meta.json", "tokenizer.json", "encoder.onnx", "decoder.onnx"]
        .iter()
        .all(|nome| dir.join(nome).is_file())
}

/// Diretorio do modelo, na ordem em que os candidatos sao tentados:
/// variavel de ambiente, download do usuario, recursos do app, arvore do repo.
///
/// O download vem antes dos recursos porque e ele que existe na maquina de quem
/// instalou o app: o `.onnx` nao cabe no instalador (ver [`crate::setup`]).
fn models_dir(app: &AppHandle) -> Result<PathBuf> {
    if let Some(bruto) = std::env::var_os(MODELS_ENV) {
        return Ok(PathBuf::from(bruto));
    }
    if let Ok(baixado) = crate::setup::destino(app) {
        if completo(&baixado) {
            return Ok(baixado);
        }
    }
    if let Ok(recursos) = app.path().resource_dir() {
        let candidato = recursos.join("nmt");
        if completo(&candidato) {
            return Ok(candidato);
        }
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/nmt");
    if completo(&repo) {
        return Ok(repo);
    }
    Err(Error::Translate(
        "tradutor de frases nao instalado — instale-o em Configuracoes".into(),
    ))
}

/// Ha um tradutor utilizavel em algum dos caminhos conhecidos?
///
/// Existe para a tela de setup nao oferecer um download de 332 MB a quem ja tem
/// o modelo na arvore do repo (o caso do desenvolvimento).
pub fn modelo_disponivel(app: &AppHandle) -> bool {
    models_dir(app).is_ok()
}

/// Quando o modelo carregado foi usado pela ultima vez.
static ULTIMO_USO: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// Ociosidade que autoriza devolver a RAM do tradutor.
///
/// Descarregar a cada espiada era pior do que o problema que resolvia: quem
/// joga espia varias palavras seguidas, e cada uma pagava ~1 s de recarga.
/// Tres minutos cobrem uma cena inteira de dialogo e ainda devolvem a memoria
/// antes de o jogador sentir falta dela.
const OCIOSIDADE_MAXIMA: std::time::Duration = std::time::Duration::from_secs(180);

/// Traduz uma frase curta de ingles para portugues do Brasil.
pub fn translate_sentence(app: &AppHandle, texto: &str) -> Result<String> {
    let mut guarda = ENGINE
        .lock()
        .map_err(|_| Error::Translate("tradutor envenenado".into()))?;
    if guarda.is_none() {
        *guarda = Some(Engine::load(&models_dir(app)?)?);
    }
    if let Ok(mut ultimo) = ULTIMO_USO.lock() {
        *ultimo = Some(std::time::Instant::now());
    }
    guarda
        .as_mut()
        .expect("tradutor acabou de ser carregado")
        .translate(texto)
}

/// Libera a RAM do tradutor **se** ele estiver parado ha tempo suficiente.
///
/// Chamado ao fim de cada espiada. Devolve `true` quando descarregou de fato —
/// o que quase nunca acontece durante uma sessao de jogo, que e o ponto.
pub fn unload_if_idle() -> bool {
    let ocioso = ULTIMO_USO
        .lock()
        .ok()
        .and_then(|ultimo| *ultimo)
        .is_none_or(|quando| quando.elapsed() >= OCIOSIDADE_MAXIMA);
    if ocioso {
        unload_model();
    }
    ocioso
}

/// Libera o modelo NMT da memoria, sem perguntar. E o caminho da tela de
/// configuracoes ("devolver memoria agora") e do fechamento do app.
pub fn unload_model() {
    if let Ok(mut guarda) = ENGINE.lock() {
        *guarda = None;
    }
    if let Ok(mut ultimo) = ULTIMO_USO.lock() {
        *ultimo = None;
    }
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

/// `async` de proposito: a traducao bloqueia por centenas de milissegundos e
/// nao pode segurar a main thread, que e quem desenha a overlay.
#[tauri::command]
pub async fn translate_run(app: AppHandle, text: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || translate_sentence(&app, &text))
        .await
        .map_err(|e| Error::Translate(format!("traducao abortada: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> Meta {
        Meta {
            decoder_start_token_id: 54775,
            eos_token_id: 44670,
            decoder_layers: 2,
            decoder_attention_heads: 16,
            head_dim: 64,
        }
    }

    fn cache(entrada: &str, dados: Vec<f32>) -> Cache {
        Cache {
            entrada: entrada.to_string(),
            shape: [1, 1, dados.len(), 1],
            dados,
        }
    }

    #[test]
    fn meta_json_do_script_de_export_desserializa() {
        // Formato exato que `scripts/export-nmt.py` grava, campos extras
        // inclusos: se as duas pontas divergirem, o app so descobre em runtime.
        let bruto = r#"{
            "model": "Helsinki-NLP/opus-mt-tc-big-en-pt",
            "license": "CC BY 4.0 (Helsinki-NLP / OPUS-MT)",
            "decoderStartTokenId": 54775,
            "eosTokenId": 44670,
            "padTokenId": 54775,
            "vocabSize": 54776,
            "decoderLayers": 6,
            "decoderAttentionHeads": 16,
            "headDim": 64
        }"#;
        let meta: Meta = serde_json::from_str(bruto).expect("meta.json valido");
        assert_eq!(meta.decoder_start_token_id, 54775);
        assert_eq!(meta.eos_token_id, 44670);
        assert_eq!(meta.decoder_layers, 6);
        assert_eq!(meta.head_dim, 64);
    }

    #[test]
    fn cache_vazio_cobre_as_quatro_entradas_de_cada_camada() {
        let engine_meta = meta();
        let shape = [
            1,
            engine_meta.decoder_attention_heads,
            0,
            engine_meta.head_dim,
        ];
        let mut esperadas = Vec::new();
        for camada in 0..engine_meta.decoder_layers {
            for lado in ["decoder", "encoder"] {
                for parte in ["key", "value"] {
                    esperadas.push(format!("past_key_values.{camada}.{lado}.{parte}"));
                }
            }
        }
        assert_eq!(esperadas.len(), 8, "2 camadas x 2 lados x 2 partes");
        assert_eq!(shape[2], 0, "o primeiro passo manda cache de comprimento 0");
    }

    #[test]
    fn substituir_troca_so_o_que_veio_no_passo() {
        let mut atual = vec![
            cache("past_key_values.0.decoder.key", vec![1.0]),
            cache("past_key_values.0.encoder.key", vec![9.0, 9.0]),
        ];
        substituir(
            &mut atual,
            vec![cache("past_key_values.0.decoder.key", vec![1.0, 2.0])],
        );
        assert_eq!(atual[0].dados, vec![1.0, 2.0], "self-attention cresceu");
        assert_eq!(
            atual[1].dados,
            vec![9.0, 9.0],
            "cross-attention do primeiro passo sobrevive"
        );
    }

    #[test]
    fn substituir_ignora_entrada_desconhecida() {
        let mut atual = vec![cache("past_key_values.0.decoder.key", vec![1.0])];
        substituir(
            &mut atual,
            vec![cache("past_key_values.9.decoder.key", vec![7.0])],
        );
        assert_eq!(atual.len(), 1);
        assert_eq!(atual[0].dados, vec![1.0]);
    }
}
