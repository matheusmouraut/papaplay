//! Da linha na tela para a frase de contexto.
//!
//! # O problema
//!
//! O card guarda a frase onde a palavra apareceu, e o tradutor traduz essa
//! frase. Mas a tela nao tem frases: tem **linhas**, quebradas onde a caixa de
//! dialogo acabou. Uma fala como
//!
//! ```text
//! I dread to think what would
//! happen if he ever found out.
//! ```
//!
//! chega ao core como duas linhas. Usar so a linha da palavra dava contextos
//! pela metade ("I dread to think what would") e traducoes truncadas — foi a
//! queixa do primeiro teste com o app rodando.
//!
//! # O criterio
//!
//! Duas etapas, nesta ordem, porque nenhuma resolve sozinha:
//!
//! 1. **Bloco:** linhas empilhadas, com alturas parecidas e margens alinhadas,
//!    sao o mesmo texto corrido. E o que junta a fala partida em duas linhas
//!    sem colar nela o nome do personagem (fonte maior) nem o item de menu do
//!    outro canto da tela.
//! 2. **Pontuacao:** dentro do bloco, corta em `.`, `!`, `?`. E o que evita
//!    mandar um paragrafo inteiro para o tradutor quando a caixa de dialogo tem
//!    tres frases.
//!
//! Se as duas falharem, sobra a propria linha — nunca menos do que hoje.

use super::BBox;

/// Vao vertical maximo entre duas linhas do mesmo bloco, em multiplos da
/// altura da linha. Acima disto e outro elemento da interface, nao continuacao.
const VAO_MAXIMO: f32 = 0.9;

/// Diferenca maxima de altura entre linhas do mesmo bloco. Fonte muito
/// diferente = papel diferente na tela (nome do personagem, titulo, legenda).
const RAZAO_DE_ALTURA: f32 = 1.6;

/// Desalinhamento horizontal tolerado entre linhas do mesmo bloco, em
/// multiplos da altura. Texto corrido tem margem comum; itens espalhados pela
/// interface, nao.
const DESALINHO: f32 = 2.5;

/// Uma linha reconhecida, do ponto de vista do agrupamento.
#[derive(Debug, Clone, Copy)]
pub struct Linha<'a> {
    pub text: &'a str,
    pub bbox: BBox,
}

/// A frase que contem a linha `indice`.
///
/// Devolve o texto ja normalizado (espacos simples, sem sobra nas pontas).
pub fn sentence_at(linhas: &[Linha<'_>], indice: usize) -> String {
    let Some(bloco) = bloco_de(linhas, indice) else {
        return String::new();
    };

    // Onde a linha pedida comeca dentro do texto do bloco: e por essa posicao
    // que se escolhe qual das frases do bloco e a certa.
    let mut posicao_da_linha = 0usize;
    let mut texto = String::new();
    for (i, linha) in bloco.clone().map(|i| linhas[i]).enumerate() {
        let pedaco = linha.text.trim();
        if pedaco.is_empty() {
            continue;
        }
        if !texto.is_empty() {
            // Hifen no fim da linha e quebra de palavra, nao separador: "under-\n
            // stand" e uma palavra so.
            if texto.ends_with('-') {
                texto.pop();
            } else {
                texto.push(' ');
            }
        }
        if bloco.start + i == indice {
            posicao_da_linha = texto.len();
        }
        texto.push_str(pedaco);
    }

    frase_em(&texto, posicao_da_linha)
}

/// Intervalo de linhas do bloco a que `indice` pertence.
fn bloco_de(linhas: &[Linha<'_>], indice: usize) -> Option<std::ops::Range<usize>> {
    if indice >= linhas.len() {
        return None;
    }
    let mut inicio = indice;
    while inicio > 0 && continua(&linhas[inicio - 1], &linhas[inicio]) {
        inicio -= 1;
    }
    let mut fim = indice;
    while fim + 1 < linhas.len() && continua(&linhas[fim], &linhas[fim + 1]) {
        fim += 1;
    }
    Some(inicio..fim + 1)
}

/// `true` se `baixo` e continuacao de `cima`.
fn continua(cima: &Linha<'_>, baixo: &Linha<'_>) -> bool {
    let (a, b) = (cima.bbox, baixo.bbox);
    if a.h == 0 || b.h == 0 {
        return false;
    }

    let (menor, maior) = if a.h < b.h { (a.h, b.h) } else { (b.h, a.h) };
    if maior as f32 > menor as f32 * RAZAO_DE_ALTURA {
        return false;
    }

    // Linha de baixo tem que estar mesmo embaixo: o detector as vezes devolve
    // duas caixas na mesma altura (texto em duas colunas).
    if b.y < a.bottom().saturating_sub(a.h / 2) {
        return false;
    }
    let vao = b.y.saturating_sub(a.bottom()) as f32;
    if vao > menor as f32 * VAO_MAXIMO {
        return false;
    }

    // Alinhamento pela esquerda ou pelo centro cobre texto justificado e texto
    // centralizado, que sao os dois jeitos de escrever dialogo em jogo.
    let esquerda = (a.x as i64 - b.x as i64).unsigned_abs() as f32;
    let centro = (a.center_x() - b.center_x()).abs();
    esquerda <= menor as f32 * DESALINHO || centro <= menor as f32 * DESALINHO
}

/// A frase de `texto` que cobre a posicao `posicao` (em bytes).
fn frase_em(texto: &str, posicao: usize) -> String {
    let bytes = texto.as_bytes();
    let posicao = posicao.min(bytes.len().saturating_sub(1));

    // Para tras ate depois de um terminador. Abreviacoes ("Mr.", "e.g.") ainda
    // cortam errado — aceitavel: o pedaco continua legivel e traduzivel, e o
    // alternativo seria uma lista de abreviacoes por idioma.
    let mut inicio = 0;
    for i in (0..posicao).rev() {
        if termina_frase(bytes[i]) && bytes.get(i + 1).is_some_and(|b| *b == b' ') {
            inicio = i + 2;
            break;
        }
    }

    let mut fim = texto.len();
    for (i, byte) in bytes.iter().enumerate().skip(posicao) {
        if termina_frase(*byte) {
            fim = i + 1;
            break;
        }
    }

    // Fatiar em byte pode cair no meio de um caractere multibyte; nesse caso
    // vale o texto inteiro, que e sempre uma resposta valida.
    if !texto.is_char_boundary(inicio) || !texto.is_char_boundary(fim) {
        return texto.trim().to_string();
    }
    texto[inicio..fim].trim().to_string()
}

fn termina_frase(byte: u8) -> bool {
    matches!(byte, b'.' | b'!' | b'?')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linha(text: &str, x: u32, y: u32, w: u32, h: u32) -> Linha<'_> {
        Linha {
            text,
            bbox: BBox { x, y, w, h },
        }
    }

    /// Fala partida em duas linhas — o caso que motivou o modulo.
    fn dialogo() -> Vec<Linha<'static>> {
        vec![
            linha("I dread to think what would", 100, 500, 400, 24),
            linha("happen if he ever found out.", 100, 528, 400, 24),
        ]
    }

    #[test]
    fn a_frase_atravessa_a_quebra_de_linha() {
        for indice in 0..2 {
            assert_eq!(
                sentence_at(&dialogo(), indice),
                "I dread to think what would happen if he ever found out.",
                "a linha {indice} tem que dar a frase inteira"
            );
        }
    }

    #[test]
    fn o_nome_do_personagem_nao_entra_na_frase() {
        // Fonte maior e outro papel na tela: juntar poria "ELDER" na frase do
        // card e na entrada do tradutor.
        let linhas = vec![
            linha("ELDER", 100, 460, 120, 40),
            linha("I dread to think what would", 100, 505, 400, 24),
            linha("happen if he ever found out.", 100, 533, 400, 24),
        ];
        let frase = sentence_at(&linhas, 1);
        assert!(!frase.contains("ELDER"), "veio: {frase}");
        assert!(frase.starts_with("I dread"), "veio: {frase}");
    }

    #[test]
    fn texto_do_outro_canto_da_tela_nao_entra() {
        let linhas = vec![
            linha("Objetivo: falar com o ferreiro", 1800, 200, 300, 22),
            linha("He ran away.", 100, 800, 200, 22),
        ];
        assert_eq!(sentence_at(&linhas, 1), "He ran away.");
    }

    #[test]
    fn duas_frases_no_mesmo_bloco_saem_separadas() {
        // O bloco inteiro seria longo demais para o card e para o tradutor.
        let linhas = vec![
            linha("He ran away. I dread to", 100, 500, 400, 24),
            linha("think what happened next.", 100, 528, 400, 24),
        ];
        assert_eq!(sentence_at(&linhas, 0), "He ran away.");
    }

    #[test]
    fn a_frase_escolhida_e_a_que_cobre_a_linha_pedida() {
        let linhas = vec![
            linha("He ran away.", 100, 500, 200, 24),
            linha("I dread the night.", 100, 528, 250, 24),
        ];
        assert_eq!(sentence_at(&linhas, 1), "I dread the night.");
    }

    #[test]
    fn palavra_quebrada_com_hifen_e_remontada() {
        let linhas = vec![
            linha("You would not under-", 100, 500, 300, 24),
            linha("stand what happened.", 100, 528, 300, 24),
        ];
        assert_eq!(
            sentence_at(&linhas, 0),
            "You would not understand what happened."
        );
    }

    #[test]
    fn texto_centralizado_conta_como_o_mesmo_bloco() {
        // Legenda e dialogo de jogo costumam ser centralizados: alinhar so pela
        // esquerda perderia justamente esses.
        let linhas = vec![
            linha("I dread to think what", 300, 500, 400, 24),
            linha("would happen next.", 350, 528, 300, 24),
        ];
        assert_eq!(
            sentence_at(&linhas, 0),
            "I dread to think what would happen next."
        );
    }

    #[test]
    fn linha_sem_pontuacao_devolve_o_bloco_inteiro() {
        // Interface sem ponto final (item de menu, HUD): nao ha onde cortar, e
        // devolver o que se tem e melhor do que devolver nada.
        let linhas = vec![linha("Press E to open", 100, 500, 200, 24)];
        assert_eq!(sentence_at(&linhas, 0), "Press E to open");
    }

    #[test]
    fn indice_fora_da_lista_nao_entra_em_panico() {
        assert_eq!(sentence_at(&dialogo(), 99), "");
        assert_eq!(sentence_at(&[], 0), "");
    }

    #[test]
    fn acento_no_texto_nao_quebra_o_fatiamento() {
        // Fatiar por byte no meio de um caractere multibyte seria panico.
        let linhas = vec![linha("Você não vai acreditar.", 100, 500, 300, 24)];
        assert_eq!(sentence_at(&linhas, 0), "Você não vai acreditar.");
    }
}
