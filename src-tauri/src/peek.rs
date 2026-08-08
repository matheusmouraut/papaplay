//! Espiada: segurar `Alt+X`, olhar a palavra, soltar.
//!
//! # Por que nao existe "modo ligado"
//!
//! A primeira versao entrava num modo lookup persistente: a overlay tomava o
//! foco, o jogo ficava atras e as palavras lidas ganhavam caixas na tela.
//! Testando com jogo de verdade isso quebra a imersao — que e exatamente o que
//! o produto promete preservar. O gesto certo e o de quem passa o dedo numa
//! palavra enquanto le: apertar, olhar, soltar. Ver F1 no `docs/03`.
//!
//! Tres estados, e so o do meio consome CPU:
//!
//! - [`Estado::Idle`] — nada. A overlay esta la, transparente e click-through.
//! - [`Estado::Peek`] — a tecla esta pressionada: um laco segue o cursor, le a
//!   tela em volta dele e avisa a UI qual palavra esta sob o mouse.
//! - [`Estado::Card`] — o usuario clicou: a overlay passa a receber cliques
//!   (sem tirar o foco do jogo) e o card fica aberto ate `Esc` ou clique fora.
//!
//! # Por que um laco proprio, e nao eventos do webview
//!
//! Espiando, a overlay e click-through: o webview **nao recebe mouse nenhum**.
//! Cursor e clique so podem vir de leitura de estado do Windows
//! ([`platform::cursor_pos`], [`platform::primary_button_down`]), num laco
//! nosso. E o preco de nao atrapalhar o jogo — e ele custa ~0,1% de CPU, so
//! enquanto a tecla esta pressionada.
//!
//! # Por que a leitura persegue o cursor
//!
//! Uma leitura cobre uma janela em volta do cursor ([`PEEK_REGION`]), nao a
//! tela inteira (spike 02: ler tudo estoura o orcamento de 1 s). Se a leitura
//! acontecesse so no instante da tecla, andar com o mouse para fora daquela
//! janela faria a ferramenta simplesmente parar de responder — foi o que
//! aconteceu no primeiro teste. Por isso [`precisa_reler`].

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::capture::{self, Region};
use crate::error::Result;
use crate::lookup::{self, LookupResult, OverlayRect};
use crate::ocr::{self, sentences, BBox};
use crate::overlay;
use crate::platform::{self, MonitorRect};

/// Janela lida a cada leitura, em pixels fisicos.
///
/// Mais baixa que a `LOOKUP_REGION` da spike (1280x720) porque espiar le
/// varias vezes: uma faixa larga e baixa pega a caixa de dialogo e as linhas
/// vizinhas, que e onde o texto de jogo vive, por ~metade do custo de OCR.
pub const PEEK_REGION: (u32, u32) = (1200, 420);

/// Distancia da borda da regiao lida que ja pede releitura, em pixels fisicos.
/// Reler antes de o cursor sair evita o buraco de "nao ha palavra aqui".
const MARGEM_DE_RELEITURA: i32 = 140;

/// Passo do laco que segue o cursor. 60 Hz e o suficiente para o tooltip
/// parecer colado no mouse sem virar consumo visivel.
const PASSO: Duration = Duration::from_millis(16);

/// Intervalo minimo entre duas leituras.
///
/// Cada leitura custa captura + OCR. Medido em release com o `ocr-spike` numa
/// regiao de 1200x420: 104 ms de media, 278 ms no pior caso (tela cheia de
/// texto). O intervalo fica um pouco acima da media — perto o bastante para a
/// leitura acompanhar o mouse, largo o bastante para atravessar a tela nao
/// enfileirar leituras.
const INTERVALO_MINIMO: Duration = Duration::from_millis(150);

/// Tempo sem palavra sob o cursor que dispara uma releitura no lugar.
///
/// Cobre o caso de a tela ter mudado embaixo da espiada (o dialogo avancou):
/// a regiao continua a mesma, mas o que estava escrito nao.
const PACIENCIA_SEM_PALAVRA: Duration = Duration::from_millis(400);

/// Confianca minima para uma palavra ser espiavel. Abaixo disto o OCR erra
/// mais do que acerta, e oferecer a traducao de um erro e pior do que nao
/// oferecer nada.
const CONF_MINIMA: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Estado {
    Idle,
    Peek,
    Card,
}

impl Estado {
    fn de_u8(valor: u8) -> Estado {
        match valor {
            1 => Estado::Peek,
            2 => Estado::Card,
            _ => Estado::Idle,
        }
    }

    fn para_u8(self) -> u8 {
        match self {
            Estado::Idle => 0,
            Estado::Peek => 1,
            Estado::Card => 2,
        }
    }
}

static ESTADO: AtomicU8 = AtomicU8::new(0);

/// Geracao da espiada atual. Encerrar incrementa, e o laco antigo percebe que
/// nao e mais o dono da tela e sai — sem precisar esperar por ele.
static GERACAO: AtomicU64 = AtomicU64::new(0);

/// Pedido de abrir o card vindo da tecla, e nao do clique.
///
/// A hotkey chega numa thread qualquer; quem sabe o alvo e a palavra sob o
/// cursor e o laco. Uma bandeira e o caminho mais curto entre as duas — e faz
/// tecla e clique desembocarem no mesmo lugar, em vez de duas rotas para o
/// mesmo card.
static PEDIU_CARD: AtomicBool = AtomicBool::new(false);

/// A palavra sob o cursor, do jeito que a UI precisa para desenhar.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Foco {
    /// Leitura de onde esta palavra saiu — o mesmo id que o salvamento usa
    /// para recortar o screenshot.
    pub lookup_id: u64,
    pub word: String,
    /// Retangulo da palavra em pixels logicos da overlay.
    pub rect: OverlayRect,
    pub line_index: usize,
    /// Frase onde a palavra esta, para o contexto do card.
    pub sentence: String,
    pub game_name: Option<String>,
}

pub fn estado() -> Estado {
    Estado::de_u8(ESTADO.load(Ordering::SeqCst))
}

fn definir_estado(app: &AppHandle, novo: Estado) {
    ESTADO.store(novo.para_u8(), Ordering::SeqCst);
    let _ = app.emit("peek://state", novo);
}

/// Comeca a espiar. Chamado quando a tecla desce.
///
/// Idempotente de proposito: o Windows repete o "pressionado" enquanto a tecla
/// fica segurada, e cada repeticao chegaria aqui.
pub fn begin(app: &AppHandle) {
    if estado() != Estado::Idle {
        return;
    }
    definir_estado(app, Estado::Peek);
    let geracao = GERACAO.fetch_add(1, Ordering::SeqCst) + 1;

    let app = app.clone();
    thread::spawn(move || {
        if let Err(erro) = seguir_o_cursor(&app, geracao) {
            // Falha tipica: modelos de OCR ausentes. Sem aviso, a espiada
            // pareceria simplesmente nao funcionar.
            eprintln!("[peek] espiada interrompida: {erro}");
            let _ = app.emit("peek://error", erro.to_string());
            end(&app);
        }
    });
}

/// A tecla foi solta.
///
/// Com um card aberto isto **nao** fecha nada: o usuario abriu o card
/// justamente para poder soltar a tecla e ler com calma.
pub fn release(app: &AppHandle) {
    if estado() == Estado::Peek {
        end(app);
    }
}

/// Abre o card da palavra sob o cursor — o caminho pelo teclado.
///
/// Existe porque o clique nem sempre pode ser impedido de chegar ao jogo
/// (`overlay::arm` explica), e num FPS clicar numa palavra tambem atira. Uma
/// hotkey e consumida pelo Windows antes de o jogo ve-la.
///
/// Fora de uma espiada nao faz nada: `Alt+C` sozinho, no meio do jogo, tem que
/// ser inofensivo.
pub fn open_card(_app: &AppHandle) {
    if estado() == Estado::Peek {
        PEDIU_CARD.store(true, Ordering::SeqCst);
    }
}

/// Volta ao repouso e devolve a memoria da sessao.
pub fn end(app: &AppHandle) {
    if estado() == Estado::Idle {
        return;
    }
    // Derruba o laco antes de mexer na janela: ele checa a geracao a cada passo.
    GERACAO.fetch_add(1, Ordering::SeqCst);
    definir_estado(app, Estado::Idle);
    // Um `Alt+C` que chegou tarde demais nao pode abrir card na espiada
    // seguinte.
    PEDIU_CARD.store(false, Ordering::SeqCst);

    if let Err(erro) = overlay::set_mode(app, false) {
        eprintln!("[peek] nao voltou para click-through: {erro}");
    }
    let _ = app.emit("peek://focus", Option::<Foco>::None);

    // O frame (~3,5 MB) vai embora sempre: sem espiada nao ha o que recortar.
    lookup::forget_capture();
    // O tradutor (~350 MB), so se estiver parado ha tempo — espiar varias
    // palavras seguidas nao pode pagar a recarga a cada uma.
    crate::translate::unload_if_idle();
    overlay::set_lookup_target(None);
}

/// O laco da espiada: segue o cursor, le a tela quando precisa, avisa a UI.
fn seguir_o_cursor(app: &AppHandle, geracao: u64) -> Result<()> {
    let alvo = platform::foreground_target()?;
    overlay::arm(app, alvo.monitor)?;
    overlay::set_lookup_target(Some(alvo.clone()));

    let mut leitura: Option<LookupResult> = None;
    let mut ultima_leitura: Option<Instant> = None;
    let mut focada: Option<Foco> = None;
    let mut sem_palavra_desde: Option<Instant> = None;
    // Se o usuario ja estava com o botao pressionado (arrastando a mira, por
    // exemplo) quando comecou a espiar, aquele clique nao e para nos.
    let mut botao_antes = platform::primary_button_down();

    while ativo(geracao) {
        let Some(cursor) = platform::cursor_pos() else {
            thread::sleep(PASSO);
            continue;
        };

        let ocioso =
            sem_palavra_desde.is_some_and(|desde| desde.elapsed() >= PACIENCIA_SEM_PALAVRA);
        let precisa = match &leitura {
            None => true,
            Some(lida) => ocioso || precisa_reler(lida.region, alvo.monitor, cursor, PEEK_REGION),
        };
        let pode = ultima_leitura.is_none_or(|quando| quando.elapsed() >= INTERVALO_MINIMO);

        if precisa && pode {
            // Uma linha so: o tooltip mostra uma traducao, e o gesto precisa
            // parecer colado no mouse. A vizinhanca (para a frase do card) so
            // e lida se o usuario clicar — ver `abrir_card`.
            let lida =
                lookup::read_around(app, &alvo, Some(cursor), PEEK_REGION, ocr::Foco::TOOLTIP)?;
            // O orcamento da F1 e o primeiro tooltip em 400 ms. Quando estoura,
            // o log diz qual das duas etapas foi — sem isso, "esta lento" vira
            // adivinhacao (foi o que aconteceu no teste de 2026-08-01).
            if lida.total_ms > 400.0 {
                eprintln!(
                    "[peek] leitura em {:.0} ms (captura {:.0} + ocr {:.0}), {} palavras",
                    lida.total_ms,
                    lida.capture_ms,
                    lida.ocr_ms,
                    lida.words.len()
                );
            }
            leitura = Some(lida);
            ultima_leitura = Some(Instant::now());
            sem_palavra_desde = None;
            if !ativo(geracao) {
                break;
            }
        }

        let novo = leitura
            .as_ref()
            .and_then(|lida| foco_em(lida, cursor, alvo.monitor, alvo.scale_factor));
        if novo.is_some() {
            sem_palavra_desde = None;
        } else if sem_palavra_desde.is_none() {
            sem_palavra_desde = Some(Instant::now());
        }
        if novo != focada {
            app.emit("peek://focus", &novo)?;
            focada = novo;
        }

        // Dois caminhos para o mesmo card: o clique (natural, mas que vaza
        // para jogos de raw input) e a tecla (sempre segura). Borda de descida
        // no clique, nao estado: segurar o botao nao pode reabrir o card a cada
        // passo do laco.
        let botao = platform::primary_button_down();
        let clicou = botao && !botao_antes;
        botao_antes = botao;

        if clicou || PEDIU_CARD.swap(false, Ordering::SeqCst) {
            if let Some(foco) = focada.clone() {
                abrir_card(app, &alvo, foco)?;
                return Ok(());
            }
        }

        thread::sleep(PASSO);
    }
    Ok(())
}

fn ativo(geracao: u64) -> bool {
    estado() == Estado::Peek && GERACAO.load(Ordering::SeqCst) == geracao
}

fn abrir_card(app: &AppHandle, alvo: &platform::ForegroundTarget, foco: Foco) -> Result<()> {
    definir_estado(app, Estado::Card);
    // Passa a receber cliques (e registra o `Esc`) sem tocar no foco do jogo.
    overlay::set_mode(app, true)?;

    // O tooltip leu uma linha; o card quer a frase, que atravessa linhas. A
    // releitura sai do frame que ja esta na memoria — capturar de novo traria
    // outra tela, porque o jogo andou entre o hover e o clique.
    let completo = ampliar(app, alvo, &foco);
    app.emit("peek://card", &completo)?;
    Ok(())
}

/// Refaz o foco com a frase inteira. Devolve o foco original se a releitura
/// falhar ou nao achar a palavra: card com contexto curto e melhor do que
/// nenhum card.
fn ampliar(app: &AppHandle, alvo: &platform::ForegroundTarget, foco: &Foco) -> Foco {
    let leitura = match lookup::expand_stored(app, foco.lookup_id, alvo) {
        Ok(Some(leitura)) => leitura,
        Ok(None) => return foco.clone(),
        Err(erro) => {
            eprintln!("[peek] releitura para o card falhou: {erro}");
            return foco.clone();
        }
    };

    let Some(cursor) = platform::cursor_pos() else {
        return foco.clone();
    };
    foco_em(&leitura, cursor, alvo.monitor, alvo.scale_factor).unwrap_or_else(|| {
        // A releitura pode nao reencontrar a palavra (o clique moveu o cursor
        // um pixel para fora da caixa). O foco do hover continua valido.
        foco.clone()
    })
}

/// Decide se vale ler de novo porque o cursor esta perto da borda do que ja
/// foi lido.
///
/// A comparacao com a regiao que *seria* lida agora resolve o caso da borda da
/// tela: la a regiao ja esta encostada e deslizar nao muda nada, entao reler
/// seria pagar OCR pelo mesmo pedaco de tela para sempre.
fn precisa_reler(
    lida: Region,
    monitor: MonitorRect,
    cursor: (i32, i32),
    tamanho: (u32, u32),
) -> bool {
    let desejada = capture::region_around(monitor, cursor, tamanho);
    if desejada == lida {
        return false;
    }
    perto_da_borda(lida, cursor)
}

fn perto_da_borda(regiao: Region, (x, y): (i32, i32)) -> bool {
    let direita = regiao.x + regiao.width as i32;
    let base = regiao.y + regiao.height as i32;
    x - regiao.x < MARGEM_DE_RELEITURA
        || direita - x < MARGEM_DE_RELEITURA
        || y - regiao.y < MARGEM_DE_RELEITURA
        || base - y < MARGEM_DE_RELEITURA
}

/// A palavra sob o cursor, se houver.
///
/// A ultima da lista vence quando duas caixas se sobrepoem: as caixas saem de
/// uma faixa do CTC (spike 02) e podem encostar nas vizinhas — a de cima e a
/// que o usuario enxerga.
fn foco_em(
    leitura: &LookupResult,
    cursor: (i32, i32),
    monitor: MonitorRect,
    escala: f64,
) -> Option<Foco> {
    let (x, y) = cursor_na_overlay(cursor, monitor, escala);
    let palavra = leitura
        .words
        .iter()
        .filter(|palavra| palavra.conf >= CONF_MINIMA)
        .rfind(|palavra| palavra.rect.contains(x, y))?;

    Some(Foco {
        lookup_id: leitura.lookup_id,
        word: palavra.text.clone(),
        rect: palavra.rect,
        line_index: palavra.line_index,
        sentence: frase_da_linha(leitura, palavra.line_index),
        game_name: leitura.game_name.clone(),
    })
}

/// A frase de contexto — a fala inteira, nao o pedaco que coube na linha.
///
/// Uma fala de jogo quase sempre ocupa duas ou tres linhas na caixa de
/// dialogo; usar so a linha da palavra dava contexto pela metade e traducao
/// truncada. Ver [`crate::ocr::sentences`].
fn frase_da_linha(leitura: &LookupResult, line_index: usize) -> String {
    // As caixas aqui ja estao em pixels logicos da overlay, e nao nos pixels
    // fisicos do recorte. Nao faz diferenca: o agrupamento so compara linhas
    // entre si, e a escala e a mesma para todas.
    let linhas: Vec<sentences::Linha<'_>> = leitura
        .lines
        .iter()
        .map(|linha| sentences::Linha {
            text: &linha.text,
            bbox: BBox {
                x: linha.rect.x.max(0.0) as u32,
                y: linha.rect.y.max(0.0) as u32,
                w: linha.rect.w.max(0.0) as u32,
                h: linha.rect.h.max(0.0) as u32,
            },
        })
        .collect();

    let frase = sentences::sentence_at(&linhas, line_index);
    if frase.is_empty() {
        // Rede de seguranca: nunca devolver menos contexto do que a propria
        // linha ja daria.
        return leitura
            .lines
            .get(line_index)
            .map(|linha| linha.text.clone())
            .unwrap_or_default();
    }
    frase
}

/// Cursor da area de trabalho virtual para pixels logicos da overlay.
pub fn cursor_na_overlay(cursor: (i32, i32), monitor: MonitorRect, escala: f64) -> (f64, f64) {
    let escala = if escala > 0.0 { escala } else { 1.0 };
    (
        (cursor.0 - monitor.x) as f64 / escala,
        (cursor.1 - monitor.y) as f64 / escala,
    )
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

/// Fecha o card e volta ao repouso. E o que o `Esc` e o clique fora chamam.
#[tauri::command]
pub fn peek_close(app: AppHandle) {
    end(&app);
}

#[tauri::command]
pub fn peek_state() -> Estado {
    estado()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookup::{LookupLine, LookupWord};

    fn monitor() -> MonitorRect {
        MonitorRect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        }
    }

    fn regiao(x: i32, y: i32) -> Region {
        Region {
            x,
            y,
            width: PEEK_REGION.0,
            height: PEEK_REGION.1,
        }
    }

    #[test]
    fn cursor_no_meio_da_regiao_nao_pede_releitura() {
        let lida = regiao(680, 510);
        let centro = (680 + 600, 510 + 210);
        assert!(!precisa_reler(lida, monitor(), centro, PEEK_REGION));
    }

    #[test]
    fn cursor_chegando_na_borda_pede_releitura() {
        // O buraco que o primeiro teste encontrou: apertar o atalho num canto e
        // levar o mouse para outro deixava de responder.
        let lida = regiao(680, 510);
        let perto_da_direita = (680 + PEEK_REGION.0 as i32 - 20, 510 + 210);
        assert!(precisa_reler(
            lida,
            monitor(),
            perto_da_direita,
            PEEK_REGION
        ));
    }

    #[test]
    fn na_borda_da_tela_nao_fica_relendo_o_mesmo_pedaco() {
        // Encostado no canto, a regiao desejada e identica a lida: reler seria
        // pagar OCR de graca, para sempre.
        let cursor = (5, 5);
        let lida = capture::region_around(monitor(), cursor, PEEK_REGION);
        assert!(!precisa_reler(lida, monitor(), cursor, PEEK_REGION));
    }

    #[test]
    fn a_escala_de_dpi_entra_na_conta_do_cursor() {
        let (x, y) = cursor_na_overlay((300, 150), monitor(), 1.5);
        assert_eq!((x, y), (200.0, 100.0));
    }

    #[test]
    fn monitor_secundario_vira_coordenada_local_da_overlay() {
        let secundario = MonitorRect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let (x, y) = cursor_na_overlay((-1920, 10), secundario, 1.0);
        assert_eq!((x, y), (0.0, 10.0));
    }

    fn leitura_de_teste() -> LookupResult {
        let palavra = |texto: &str, x: f64, conf: f32| LookupWord {
            text: texto.into(),
            rect: OverlayRect {
                x,
                y: 100.0,
                w: 40.0,
                h: 20.0,
            },
            conf,
            line_index: 0,
        };
        LookupResult {
            lookup_id: 7,
            words: vec![
                palavra("He", 10.0, 0.9),
                palavra("dreads", 60.0, 0.9),
                palavra("ruido", 200.0, 0.2),
            ],
            lines: vec![LookupLine {
                text: "He dreads the night.".into(),
                rect: OverlayRect {
                    x: 10.0,
                    y: 100.0,
                    w: 300.0,
                    h: 20.0,
                },
            }],
            cursor: None,
            game_name: Some("Skyrim".into()),
            region: regiao(0, 0),
            capture_ms: 0.0,
            ocr_ms: 0.0,
            total_ms: 0.0,
        }
    }

    #[test]
    fn a_palavra_sob_o_cursor_vem_com_a_frase_dela() {
        let foco = foco_em(&leitura_de_teste(), (70, 110), monitor(), 1.0).expect("achou");
        assert_eq!(foco.word, "dreads");
        assert_eq!(foco.sentence, "He dreads the night.");
        assert_eq!(foco.lookup_id, 7);
        assert_eq!(foco.game_name.as_deref(), Some("Skyrim"));
    }

    #[test]
    fn a_frase_do_foco_atravessa_a_quebra_de_linha() {
        // A queixa do primeiro teste: contexto e traducao chegavam pela metade
        // quando a fala ocupava duas linhas da caixa de dialogo.
        let mut leitura = leitura_de_teste();
        let caixa = |y: f64, w: f64| OverlayRect {
            x: 10.0,
            y,
            w,
            h: 20.0,
        };
        leitura.lines = vec![
            LookupLine {
                text: "I dread to think what would".into(),
                rect: caixa(100.0, 300.0),
            },
            LookupLine {
                text: "happen if he ever found out.".into(),
                rect: caixa(122.0, 300.0),
            },
        ];
        let foco = foco_em(&leitura, (70, 110), monitor(), 1.0).expect("achou");
        assert_eq!(
            foco.sentence,
            "I dread to think what would happen if he ever found out."
        );
    }

    #[test]
    fn fora_de_qualquer_palavra_nao_ha_foco() {
        assert!(foco_em(&leitura_de_teste(), (500, 500), monitor(), 1.0).is_none());
    }

    #[test]
    fn palavra_de_baixa_confianca_nao_e_espiavel() {
        // Traduzir um erro de OCR e pior do que nao mostrar nada.
        assert!(foco_em(&leitura_de_teste(), (210, 110), monitor(), 1.0).is_none());
    }

    #[test]
    fn estado_sobrevive_a_ida_e_volta_pelo_inteiro() {
        for estado in [Estado::Idle, Estado::Peek, Estado::Card] {
            assert_eq!(Estado::de_u8(estado.para_u8()), estado);
        }
    }
}
