//! Agrupamento das caixas detectadas em linhas de leitura.
//!
//! O detector ja devolve uma caixa por linha na maioria dos casos, mas ele
//! quebra a linha quando algo interrompe o texto — um icone de tecla no meio
//! da frase, uma palavra colorida com contorno diferente, um espaco grande.
//! Sem juntar de volta, a frase de contexto que vai para o card sai picotada.
//!
//! Criterio: duas caixas sao a mesma linha quando se sobrepoem verticalmente
//! o bastante **e** o vao horizontal entre elas cabe em poucos "corpos de
//! letra" (multiplos da altura da caixa).

use super::detect::DetectParams;
use super::BBox;

/// Junta as caixas em grupos de leitura. A entrada deve vir ordenada de cima
/// para baixo e da esquerda para a direita.
pub fn group(caixas: Vec<BBox>, params: &DetectParams) -> Vec<Vec<BBox>> {
    let mut grupos: Vec<Vec<BBox>> = Vec::new();

    for caixa in caixas {
        let alvo = grupos.iter_mut().find(|grupo| {
            let ultima = grupo.last().expect("grupo nunca e criado vazio");
            mesma_linha(ultima, &caixa, params)
        });

        match alvo {
            Some(grupo) => grupo.push(caixa),
            None => grupos.push(vec![caixa]),
        }
    }

    // Dentro da linha, ordem de leitura; entre linhas, de cima para baixo.
    for grupo in grupos.iter_mut() {
        grupo.sort_by_key(|c| c.x);
    }
    grupos.sort_by(|a, b| {
        let ea = envelope(a);
        let eb = envelope(b);
        ea.y.cmp(&eb.y).then(ea.x.cmp(&eb.x))
    });
    grupos
}

fn mesma_linha(esquerda: &BBox, direita: &BBox, params: &DetectParams) -> bool {
    if esquerda.vertical_overlap(direita) < params.line_overlap {
        return false;
    }
    // Caixas de alturas muito diferentes sao coisas diferentes (titulo x
    // corpo), mesmo que se sobreponham.
    let (menor, maior) = if esquerda.h < direita.h {
        (esquerda.h, direita.h)
    } else {
        (direita.h, esquerda.h)
    };
    if maior as f32 > menor as f32 * 2.0 {
        return false;
    }

    let vao = direita.x.saturating_sub(esquerda.right());
    vao as f32 <= menor as f32 * params.line_gap_ratio
}

/// Menor retangulo que contem todas as caixas do grupo.
pub fn envelope(grupo: &[BBox]) -> BBox {
    let mut iter = grupo.iter();
    let Some(primeira) = iter.next() else {
        return BBox {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
    };

    let mut x0 = primeira.x;
    let mut y0 = primeira.y;
    let mut x1 = primeira.right();
    let mut y1 = primeira.bottom();

    for caixa in iter {
        x0 = x0.min(caixa.x);
        y0 = y0.min(caixa.y);
        x1 = x1.max(caixa.right());
        y1 = y1.max(caixa.bottom());
    }

    BBox {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(x: u32, y: u32, w: u32, h: u32) -> BBox {
        BBox { x, y, w, h }
    }

    #[test]
    fn caixas_vizinhas_na_mesma_altura_viram_uma_linha() {
        // Caso do icone no meio da frase: "Press [Shift] or ... to dash".
        let caixas = vec![bbox(0, 100, 60, 20), bbox(70, 100, 80, 20)];
        let grupos = group(caixas, &DetectParams::default());
        assert_eq!(grupos.len(), 1);
        assert_eq!(grupos[0].len(), 2);
    }

    #[test]
    fn caixas_em_alturas_diferentes_ficam_em_linhas_diferentes() {
        let caixas = vec![bbox(0, 100, 60, 20), bbox(0, 200, 60, 20)];
        assert_eq!(group(caixas, &DetectParams::default()).len(), 2);
    }

    #[test]
    fn vao_horizontal_grande_separa_as_linhas() {
        // Duas colunas da mesma tela (menu a esquerda, detalhe a direita) nao
        // podem virar uma frase so.
        let caixas = vec![bbox(0, 100, 60, 20), bbox(600, 100, 80, 20)];
        assert_eq!(group(caixas, &DetectParams::default()).len(), 2);
    }

    #[test]
    fn alturas_muito_diferentes_nao_se_juntam() {
        // Titulo grudado no corpo do texto: mesma faixa vertical, tamanhos
        // incompativeis.
        let caixas = vec![bbox(0, 100, 60, 60), bbox(70, 130, 60, 15)];
        assert_eq!(group(caixas, &DetectParams::default()).len(), 2);
    }

    #[test]
    fn grupos_saem_ordenados_de_cima_para_baixo() {
        let caixas = vec![bbox(0, 300, 60, 20), bbox(0, 100, 60, 20)];
        let grupos = group(caixas, &DetectParams::default());
        assert_eq!(grupos[0][0].y, 100);
        assert_eq!(grupos[1][0].y, 300);
    }

    #[test]
    fn dentro_da_linha_a_ordem_e_da_esquerda_para_a_direita() {
        let caixas = vec![bbox(70, 100, 60, 20), bbox(0, 100, 60, 20)];
        let grupos = group(caixas, &DetectParams::default());
        assert_eq!(grupos[0][0].x, 0);
        assert_eq!(grupos[0][1].x, 70);
    }

    #[test]
    fn envelope_cobre_todas_as_caixas() {
        let e = envelope(&[bbox(10, 20, 30, 40), bbox(100, 10, 20, 20)]);
        assert_eq!(e.x, 10);
        assert_eq!(e.y, 10);
        assert_eq!(e.right(), 120);
        assert_eq!(e.bottom(), 60);
    }

    #[test]
    fn envelope_de_grupo_vazio_e_degenerado_em_vez_de_panico() {
        assert_eq!(envelope(&[]).w, 0);
    }
}
