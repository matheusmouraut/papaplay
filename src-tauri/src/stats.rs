//! Estatisticas do deck (F6): o que foi capturado, o que foi revisado e ha
//! quantos dias seguidos.
//!
//! # O fuso vem da UI
//!
//! O banco guarda tudo em UTC, mas "hoje" e "dias seguidos" sao perguntas sobre
//! o dia **local** — quem revisa as 22h de Brasilia nao quer ver a revisao
//! contada no dia seguinte. A UI manda o deslocamento em minutos
//! (`-new Date().getTimezoneOffset()`), que vira um modificador do SQLite
//! (`'-180 minutes'`) aplicado dentro de `date()`. O horario de verao muda esse
//! numero, e a UI o recalcula a cada abertura da tela — melhor que o core
//! tentar adivinhar.
//!
//! # Por que so leitura
//!
//! Nada aqui deriva estado novo: sao contagens sobre `cards`, `contexts` e
//! `review_log`. Streak nao e um campo persistido justamente por isso — um
//! contador guardado desincroniza; uma contagem sobre o log nao tem como.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::deck::no_banco;
use crate::error::Result;

/// Teto de dias distintos com revisao que a consulta do streak traz. Tres anos
/// de uso diario — quem passar disso ve o streak saturado em 1000, o que e um
/// problema bom de ter.
const TETO_DE_DIAS_COM_REVISAO: u32 = 1000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    /// Instante de agora em ISO-8601 UTC.
    pub now: String,
    /// Deslocamento do fuso local em minutos (Brasilia = -180).
    pub tz_offset_minutes: i32,
    /// Tamanho da janela do grafico e da taxa de acerto, em dias.
    pub days: u32,
}

/// Um dia do grafico.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPoint {
    /// `YYYY-MM-DD` no fuso local.
    pub day: String,
    /// Cards criados nesse dia.
    pub created: u32,
    /// Revisoes feitas nesse dia (nao cards distintos: uma repeticao no mesmo
    /// dia e trabalho feito duas vezes).
    pub reviewed: u32,
}

/// Quantos cards vieram de cada jogo.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCount {
    pub game: String,
    pub cards: u32,
}

/// Distribuicao do deck pelos estados do FSRS.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateCounts {
    pub new: u32,
    pub learning: u32,
    pub review: u32,
    pub relearning: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Cards no deck, suspensos incluidos.
    pub total: u32,
    pub suspended: u32,
    pub states: StateCounts,
    /// Vencidos agora (sem contar os novos, que dependem da cota do dia).
    pub due_now: u32,
    pub reviewed_today: u32,
    /// Acertos / revisoes na janela. `None` quando nao houve revisao nenhuma —
    /// mostrar "0%" para quem nunca revisou seria uma acusacao falsa.
    pub accuracy: Option<f64>,
    pub reviews_in_window: u32,
    /// Dias seguidos com pelo menos uma revisao, terminando hoje ou ontem.
    pub streak: u32,
    /// Um ponto por dia da janela, do mais antigo ao mais recente.
    pub daily: Vec<DailyPoint>,
    /// Jogos com mais cards primeiro.
    pub by_game: Vec<GameCount>,
}

/// Modificador de fuso do SQLite a partir dos minutos que a UI mandou.
fn fuso(minutos: i32) -> String {
    format!("{minutos} minutes")
}

fn contar(conexao: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<u32> {
    Ok(conexao.query_row(sql, params, |linha| linha.get(0))?)
}

fn estados(conexao: &Connection) -> Result<StateCounts> {
    let mut stmt = conexao.prepare("SELECT fsrs_state, COUNT(*) FROM cards GROUP BY fsrs_state")?;
    let mut contagem = StateCounts::default();
    let linhas = stmt.query_map([], |linha| {
        Ok((linha.get::<_, String>(0)?, linha.get::<_, u32>(1)?))
    })?;
    for linha in linhas {
        let (estado, quantos) = linha?;
        match estado.as_str() {
            "new" => contagem.new = quantos,
            "learning" => contagem.learning = quantos,
            "review" => contagem.review = quantos,
            "relearning" => contagem.relearning = quantos,
            // Estado desconhecido so apareceria com o banco editado a mao;
            // ignorar mantem a tela de pe em vez de derrubar a consulta.
            _ => {}
        }
    }
    Ok(contagem)
}

/// Serie densa: um ponto por dia da janela, inclusive os dias sem nada.
///
/// A serie e gerada no SQL (CTE recursiva) em vez de no Rust porque aritmetica
/// de calendario com fuso e horario de verao ja esta resolvida dentro do
/// `date()` do SQLite — reimplementar aqui seria reintroduzir os bugs dela.
fn serie_diaria(conexao: &Connection, consulta: &StatsQuery) -> Result<Vec<DailyPoint>> {
    let janela = consulta.days.max(1);
    let mut stmt = conexao.prepare(
        "WITH RECURSIVE dias(dia) AS (
             SELECT date(:agora, :fuso, '-' || (:janela - 1) || ' days')
             UNION ALL
             SELECT date(dia, '+1 day') FROM dias WHERE dia < date(:agora, :fuso)
         )
         SELECT dia,
                (SELECT COUNT(*) FROM cards c
                  WHERE date(c.created_at, :fuso) = dia)      AS criados,
                (SELECT COUNT(*) FROM review_log r
                  WHERE date(r.reviewed_at, :fuso) = dia)     AS revisados
           FROM dias ORDER BY dia",
    )?;
    let pontos = stmt.query_map(
        rusqlite::named_params! {
            ":agora": consulta.now,
            ":fuso": fuso(consulta.tz_offset_minutes),
            ":janela": janela,
        },
        |linha| {
            Ok(DailyPoint {
                day: linha.get(0)?,
                created: linha.get(1)?,
                reviewed: linha.get(2)?,
            })
        },
    )?;
    Ok(pontos.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Dias seguidos com revisao, terminando hoje ou ontem.
///
/// Ontem conta como fim valido de propósito: a sequencia so quebra depois de um
/// dia inteiro sem revisar. Se ela zerasse a cada meia-noite, abrir o app de
/// manha mostraria "0 dias" para quem revisou ontem a noite.
fn streak(conexao: &Connection, consulta: &StatsQuery) -> Result<u32> {
    let fuso = fuso(consulta.tz_offset_minutes);
    // Dias em numero juliano: a diferenca entre dois deles e a distancia em
    // dias, sem precisar de aritmetica de calendario no Rust.
    let hoje: i64 = conexao.query_row(
        "SELECT CAST(julianday(date(?1, ?2)) AS INTEGER)",
        rusqlite::params![consulta.now, fuso],
        |linha| linha.get(0),
    )?;

    let mut stmt = conexao.prepare(
        "SELECT DISTINCT CAST(julianday(date(reviewed_at, ?1)) AS INTEGER) AS dia
           FROM review_log ORDER BY dia DESC LIMIT ?2",
    )?;
    let dias = stmt
        .query_map(rusqlite::params![fuso, TETO_DE_DIAS_COM_REVISAO], |linha| {
            linha.get::<_, i64>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let Some(&mais_recente) = dias.first() else {
        return Ok(0);
    };
    if mais_recente < hoje - 1 {
        return Ok(0);
    }

    let mut esperado = mais_recente;
    let mut total = 0;
    for dia in dias {
        if dia != esperado {
            break;
        }
        total += 1;
        esperado -= 1;
    }
    Ok(total)
}

/// Cards por jogo, contando cada card uma vez por jogo em que ele apareceu.
///
/// Um card encontrado em dois jogos conta nos dois: a pergunta que a tela
/// responde e "quanto vocabulario este jogo me deu", nao "de quem e este card".
fn por_jogo(conexao: &Connection) -> Result<Vec<GameCount>> {
    let mut stmt = conexao.prepare(
        "SELECT game_name, COUNT(DISTINCT card_id) AS cards
           FROM contexts
          WHERE game_name IS NOT NULL AND game_name <> ''
          GROUP BY game_name
          ORDER BY cards DESC, game_name COLLATE NOCASE",
    )?;
    let jogos = stmt.query_map([], |linha| {
        Ok(GameCount {
            game: linha.get(0)?,
            cards: linha.get(1)?,
        })
    })?;
    Ok(jogos.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn summary(conexao: &Connection, consulta: &StatsQuery) -> Result<Summary> {
    let fuso = fuso(consulta.tz_offset_minutes);
    let janela = consulta.days.max(1);
    let inicio_da_janela = format!("-{} days", janela - 1);

    let total = contar(conexao, "SELECT COUNT(*) FROM cards", [])?;
    let suspended = contar(
        conexao,
        "SELECT COUNT(*) FROM cards WHERE suspended = 1",
        [],
    )?;
    let due_now = contar(
        conexao,
        "SELECT COUNT(*) FROM cards
          WHERE suspended = 0 AND fsrs_state <> 'new' AND fsrs_due <= ?1",
        [&consulta.now],
    )?;
    let reviewed_today = contar(
        conexao,
        "SELECT COUNT(*) FROM review_log
          WHERE date(reviewed_at, ?1) = date(?2, ?1)",
        rusqlite::params![fuso, consulta.now],
    )?;

    // A janela da taxa de acerto e a mesma do grafico: dois numeros na mesma
    // tela medindo periodos diferentes seria uma armadilha de leitura.
    let (acertos, revisoes): (u32, u32) = conexao.query_row(
        "SELECT COALESCE(SUM(CASE WHEN rating > 1 THEN 1 ELSE 0 END), 0), COUNT(*)
           FROM review_log
          WHERE date(reviewed_at, ?1) >= date(?2, ?1, ?3)",
        rusqlite::params![fuso, consulta.now, inicio_da_janela],
        |linha| Ok((linha.get(0)?, linha.get(1)?)),
    )?;

    Ok(Summary {
        total,
        suspended,
        states: estados(conexao)?,
        due_now,
        reviewed_today,
        accuracy: (revisoes > 0).then(|| f64::from(acertos) / f64::from(revisoes)),
        reviews_in_window: revisoes,
        streak: streak(conexao, consulta)?,
        daily: serie_diaria(conexao, consulta)?,
        by_game: por_jogo(conexao)?,
    })
}

#[tauri::command]
pub async fn stats_summary(app: AppHandle, query: StatsQuery) -> Result<Summary> {
    no_banco(app, move |_, conexao| summary(conexao, &query)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn banco() -> Connection {
        let conexao = Connection::open_in_memory().expect("abriu em memoria");
        db::preparar(&conexao).expect("migrou");
        conexao
    }

    /// Consulta ancorada num instante fixo, em UTC, para os testes nao
    /// dependerem do relogio nem do fuso da maquina.
    fn consulta() -> StatsQuery {
        StatsQuery {
            now: "2026-08-08T18:00:00Z".into(),
            tz_offset_minutes: 0,
            days: 7,
        }
    }

    fn card(conexao: &Connection, lemma: &str, estado: &str, due: &str, criado: &str) -> i64 {
        conexao
            .execute(
                "INSERT INTO cards (
                     lemma, created_at, suspended, fsrs_due, fsrs_stability,
                     fsrs_difficulty, fsrs_state, fsrs_reps, fsrs_lapses,
                     fsrs_scheduled_days, fsrs_learning_steps, fsrs_last_review
                 ) VALUES (?1, ?2, 0, ?3, 1.0, 5.0, ?4, 0, 0, 0, 0, NULL)",
                rusqlite::params![lemma, criado, due, estado],
            )
            .expect("inseriu o card");
        conexao.last_insert_rowid()
    }

    fn revisao(conexao: &Connection, card_id: i64, quando: &str, rating: u8) {
        conexao
            .execute(
                "INSERT INTO review_log (
                     card_id, reviewed_at, rating, elapsed_days, state_before, state_after
                 ) VALUES (?1, ?2, ?3, 1.0, 'review', 'review')",
                rusqlite::params![card_id, quando, rating],
            )
            .expect("inseriu a revisao");
    }

    #[test]
    fn deck_vazio_nao_inventa_numeros() {
        let conexao = banco();
        let s = summary(&conexao, &consulta()).expect("resumiu");
        assert_eq!(s.total, 0);
        assert_eq!(s.streak, 0);
        assert_eq!(s.accuracy, None);
        assert_eq!(s.daily.len(), 7);
    }

    #[test]
    fn a_serie_cobre_a_janela_inteira_inclusive_os_dias_vazios() {
        let conexao = banco();
        card(
            &conexao,
            "dread",
            "new",
            "2026-08-08T00:00:00Z",
            "2026-08-06T10:00:00Z",
        );
        let s = summary(&conexao, &consulta()).expect("resumiu");
        assert_eq!(s.daily.len(), 7);
        assert_eq!(s.daily.first().map(|d| d.day.as_str()), Some("2026-08-02"));
        assert_eq!(s.daily.last().map(|d| d.day.as_str()), Some("2026-08-08"));
        let dia_do_card = s.daily.iter().find(|d| d.day == "2026-08-06").expect("dia");
        assert_eq!(dia_do_card.created, 1);
    }

    #[test]
    fn vencidos_contam_e_novos_nao() {
        let conexao = banco();
        card(
            &conexao,
            "dread",
            "review",
            "2026-08-07T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        card(
            &conexao,
            "grim",
            "new",
            "2026-08-01T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        card(
            &conexao,
            "later",
            "review",
            "2026-09-01T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        let s = summary(&conexao, &consulta()).expect("resumiu");
        assert_eq!(s.due_now, 1);
        assert_eq!(s.states.new, 1);
        assert_eq!(s.states.review, 2);
    }

    #[test]
    fn a_taxa_de_acerto_ignora_o_errei() {
        let conexao = banco();
        let id = card(
            &conexao,
            "dread",
            "review",
            "2026-08-09T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        revisao(&conexao, id, "2026-08-08T10:00:00Z", 3);
        revisao(&conexao, id, "2026-08-08T11:00:00Z", 1);
        let s = summary(&conexao, &consulta()).expect("resumiu");
        assert_eq!(s.reviews_in_window, 2);
        assert_eq!(s.accuracy, Some(0.5));
        assert_eq!(s.reviewed_today, 2);
    }

    #[test]
    fn tres_dias_seguidos_dao_streak_de_tres() {
        let conexao = banco();
        let id = card(
            &conexao,
            "dread",
            "review",
            "2026-08-09T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        for dia in ["2026-08-06", "2026-08-07", "2026-08-08"] {
            revisao(&conexao, id, &format!("{dia}T10:00:00Z"), 3);
        }
        assert_eq!(summary(&conexao, &consulta()).expect("resumiu").streak, 3);
    }

    #[test]
    fn revisar_ontem_mantem_a_sequencia_viva() {
        let conexao = banco();
        let id = card(
            &conexao,
            "dread",
            "review",
            "2026-08-09T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        revisao(&conexao, id, "2026-08-06T10:00:00Z", 3);
        revisao(&conexao, id, "2026-08-07T10:00:00Z", 3);
        assert_eq!(summary(&conexao, &consulta()).expect("resumiu").streak, 2);
    }

    #[test]
    fn um_dia_inteiro_sem_revisar_quebra_a_sequencia() {
        let conexao = banco();
        let id = card(
            &conexao,
            "dread",
            "review",
            "2026-08-09T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        revisao(&conexao, id, "2026-08-05T10:00:00Z", 3);
        revisao(&conexao, id, "2026-08-06T10:00:00Z", 3);
        assert_eq!(summary(&conexao, &consulta()).expect("resumiu").streak, 0);
    }

    #[test]
    fn o_fuso_local_decide_a_que_dia_a_revisao_pertence() {
        let conexao = banco();
        let id = card(
            &conexao,
            "dread",
            "review",
            "2026-08-09T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        // 01:00 UTC do dia 9 e ainda 22:00 do dia 8 em Brasilia.
        revisao(&conexao, id, "2026-08-09T01:00:00Z", 3);
        let brasilia = StatsQuery {
            now: "2026-08-09T02:00:00Z".into(),
            tz_offset_minutes: -180,
            days: 7,
        };
        let s = summary(&conexao, &brasilia).expect("resumiu");
        let dia = s.daily.iter().find(|d| d.day == "2026-08-08").expect("dia");
        assert_eq!(dia.reviewed, 1);
        assert_eq!(s.reviewed_today, 1);
    }

    #[test]
    fn cards_por_jogo_contam_o_card_uma_vez_em_cada_jogo() {
        let conexao = banco();
        let id = card(
            &conexao,
            "dread",
            "new",
            "2026-08-08T00:00:00Z",
            "2026-08-01T00:00:00Z",
        );
        for (frase, jogo) in [
            ("A dread silence.", "Skyrim"),
            ("Dread it.", "Skyrim"),
            ("The dread lord.", "Elden Ring"),
        ] {
            conexao
                .execute(
                    "INSERT INTO contexts (card_id, form, sentence_en, game_name, captured_at)
                     VALUES (?1, 'dread', ?2, ?3, '2026-08-08T10:00:00Z')",
                    rusqlite::params![id, frase, jogo],
                )
                .expect("inseriu contexto");
        }
        let s = summary(&conexao, &consulta()).expect("resumiu");
        assert_eq!(
            s.by_game,
            vec![
                GameCount {
                    game: "Elden Ring".into(),
                    cards: 1
                },
                GameCount {
                    game: "Skyrim".into(),
                    cards: 1
                },
            ]
        );
    }
}
