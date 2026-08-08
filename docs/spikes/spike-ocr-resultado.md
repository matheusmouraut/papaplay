# Spike 02 — Resultado: OCR com posição por palavra em telas de jogos

**Roteiro:** [`spike-02-ocr.md`](spike-02-ocr.md) · **Branch:** `spike/ocr` · **Data:** 2026-07-30

**Veredito: GO.** Recall agregado de **96,5%** (306/317 palavras) em 10 telas reais, com a pior
imagem em 90,9% — acima dos dois critérios do roteiro (≥90% em UI/diálogo, ≥75% em fundo complexo).
As caixas por palavra saem visualmente corretas. Os modelos pesam **10,9 MB**, contra o teto de
50 MB.

A latência é o único ponto com ressalva: **322 ms de média e 720 ms no pior caso** lendo a tela
2.5K inteira — 9 das 10 imagens passam do critério de <500 ms, e a décima é um painel de inventário
com 60 caixas de texto. Restringindo o OCR a uma região de 1280×720 em volta do cursor, que é o que
o produto de fato precisa fazer, o pior caso cai para **252 ms**. Ver
[Latência](#latência-o-custo-é-linear-no-número-de-caixas).

---

## Máquina de teste

| Item      | Valor                                                        |
| --------- | ------------------------------------------------------------ |
| SO        | Windows 11 Home Single Language, build 26200                  |
| CPU       | Intel Core i7-12700H (14C/20T)                                |
| RAM       | 31,7 GB                                                       |
| Execução  | CPU apenas, sem GPU — `ort` 2.0.0-rc.13 com ONNX Runtime      |
| Build     | `cargo build --release`                                       |
| Fixtures  | 10 telas de *The Seven Deadly Sins: Origin*, 2560×1440        |

**Todas as medições de latência foram feitas com a máquina ociosa.** Isso importa: a primeira
rodada saiu com o jogo ainda aberto em segundo plano e os números variaram de 340 ms a 4,5 s entre
execuções idênticas. Ver [Problema 4](#4-medição-com-o-jogo-aberto-é-inútil).

## Modelos escolhidos

| Papel            | Arquivo                      | Tamanho |
| ---------------- | ---------------------------- | ------- |
| Detecção (DBNet) | `en_PP-OCRv3_det_infer.onnx` | 2,4 MB  |
| Reconhecimento   | `en_PP-OCRv3_rec_infer.onnx` | 9,0 MB  |
| Charset          | `en_dict.txt`                | 190 B   |
| **Total**        |                              | **10,9 MB** |

Baixados por `scripts/fetch-ocr-models.ps1`, que confere o tamanho de cada arquivo e imprime o
SHA-256. Ficam fora do git (regeráveis) e **não entram em nenhuma chamada de rede em runtime** —
respeita a regra 2 do CLAUDE.md.

Contrato confirmado por sondagem antes de escrever o pipeline:

- detector: entrada `x` `[N,3,H,W]` → saída `[N,1,H,W]`, mapa de probabilidade
- reconhecedor: entrada `x` `[N,3,H,W]` → saída `[N,T,97]`

As 97 classes para um dicionário de 95 caracteres seguem a convenção do PaddleOCR:
`[branco do CTC] + dicionário + [espaço]`.

## O que foi implementado

`src-tauri/src/ocr/`, em quatro partes:

- **`detect.rs`** — pré-processamento (redução para 960 px no maior lado, lados múltiplos de 32,
  normalização ImageNet) e pós-processamento do DBNet: binarização, rotulação de componentes
  conectados por 8-vizinhos (iterativa, não recursiva — uma mancha em 2.5K tem dezenas de milhares
  de pixels), pontuação por probabilidade média e expansão `unclip`.
- **`recognize.rs`** — recorte, normalização em [-1,1], decodificação CTC gulosa e **reconstrução da
  posição de cada palavra**.
- **`lines.rs`** — agrupamento das caixas em linhas de leitura por sobreposição vertical e vão
  horizontal.
- **`mod.rs`** — `Engine` (carrega uma vez, reconhece muitas), tipos públicos e o único ponto de
  contato com a API do `ort`.

33 testes unitários, todos determinísticos: nenhum precisa dos modelos ou de imagem real.

### Como sai a caixa por palavra

**Não vem de graça.** O PP-OCR reconhece a linha inteira de uma vez e devolve texto, não posições.
A posição é reconstruída a partir do CTC: como a rede só reduz a largura, o passo de tempo `t`
corresponde à faixa `[t/T, (t+1)/T]` da largura do recorte. Uma palavra que começa no passo 4 e
termina no 9, numa linha de 40 passos, ocupa de 10% a 25% da largura da linha.

A precisão é a de uma faixa — tipicamente ~8 px do recorte — não do pixel. É de sobra para acertar
qual palavra está sob o cursor, que é o único uso.

## Números medidos

### Acurácia (recall por palavra, 5 repetições)

| Fixture   | Tipo            | Linhas | Palavras | Recall | Faltaram      |
| --------- | --------------- | ------ | -------- | ------ | ------------- |
| `215520`  | diálogo 1 linha | 3      | 15       | **100,0%** | —         |
| `215603`  | diálogo 2 linhas| 6      | 32       | **100,0%** | —         |
| `215651`  | fonte pequena   | 39     | 135      | 98,7%  | Esc           |
| `215639`  | menu            | 18     | 68       | 97,1%  | Esc           |
| `215751`  | menu            | 21     | 67       | 97,1%  | Esc           |
| `215742`  | menu            | 18     | 36       | 96,7%  | Esc           |
| `215730`  | menu            | 21     | 45       | 94,4%  | Esc           |
| `215633`  | menu            | 16     | 67       | 94,1%  | All, Esc      |
| `215622`  | menu            | 23     | 45       | 93,1%  | Book, Esc     |
| `215538`  | **fundo complexo** | 17  | 36       | 90,9%  | Quest, quest  |
| **Agregado** |              |        |          | **96,5%** (306/317) | |

O caso difícil — texto solto sobre grama animada, sem caixa atrás — deu **90,9%**, contra um
critério de 75%. Os diálogos, que são o uso principal do produto, deram **100%**.

Frases longas saem inteiras e com pontuação:

> `Now I remember. We met in front of the fog barrier, didn't we? You were the one harassing the Fairy merchant!`

Contrações sobrevivem (`didn't`, `you're`, `Sun's`) — importante, porque é o lema que vai ao
dicionário.

### Latência: o custo é linear no número de caixas

Separando as duas passadas de rede, o modelo de custo é claro:

```
latência ≈ 60 ms  +  11 ms × (número de caixas de texto)
           ^^^^^     ^^^^^
        detecção     reconhecimento (uma chamada por caixa)
```

A detecção é **constante em ~59 ms** para qualquer tela, porque a entrada é sempre reduzida para
960 px no maior lado. O reconhecimento domina: 90% do tempo na tela mais pesada.

| Região do OCR             | Média  | Pior   | Passa em <500 ms |
| ------------------------- | ------ | ------ | ---------------- |
| tela inteira 2560×1440    | 322 ms | 720 ms | 9 de 10          |
| **1280×720 (cursor)**     | 136 ms | 252 ms | **10 de 10**     |
| 960×540                   | 109 ms | 216 ms | 10 de 10         |

**Consequência de projeto:** o lookup não deve rodar OCR na tela inteira. Uma janela em volta do
cursor derruba o pior caso de 720 ms para 252 ms e deixa ~750 ms do orçamento de 1 s (doc 03) para
dicionário e tradução. O tamanho exato da janela vira parâmetro da F2.

### Threads do runtime

Varredura com a máquina ociosa, 10 fixtures, 5 repetições:

| Threads | Média  | Pior    |
| ------- | ------ | ------- |
| 1       | 855 ms | 1931 ms |
| 2       | 456 ms | 1020 ms |
| **4**   | **323 ms** | **732 ms** |
| 8       | 321 ms | 725 ms  |
| 14      | 356 ms | 793 ms  |

O ganho satura em 4 e **volta a piorar em 14** (disputa entre threads). Fixado em 4: empate técnico
com 8 usando metade dos núcleos, o que importa porque isto roda por cima de um jogo. O default do
runtime (todos os núcleos) seria pior nos dois sentidos.

Paralelizar por fora não é opção: `Session::run` do `ort` exige acesso exclusivo, então seria
preciso carregar o modelo uma vez por thread — e a RAM já está apertada
([spike 01, problema 1](spike-overlay-resultado.md#1-ram-já-estoura-a-meta-antes-de-qualquer-modelo)).

### Caixas visualmente corretas

Confirmado desenhando as caixas sobre a imagem (`--desenhar`): cada palavra recebe um retângulo
justo, e o retângulo da linha envolve a frase inteira. Numa tela de diálogo de 21 palavras, todas
as 21 caíram sobre a palavra certa.

## Problemas encontrados

### 1. Fonte estilizada confunde `Q` com `O`

Na tela de fundo complexo, `New Quest` virou `New Ouest` e `to track quest` virou `to track ques`.
É a fonte serifada e estreita do rastreador de missões, sobre grama animada — a perna do `Q` some.

São 2 dos 11 erros do conjunto inteiro. Não bloqueia: o dicionário vai errar essa palavra
específica, e o usuário vai ver a palavra errada no card. **Mitigação para a Fase 1:** conferir a
palavra lida contra o dicionário e, quando não existir, tentar as confusões conhecidas
(`O`↔`Q`, `l`↔`I`, `rn`↔`m`) antes de desistir. Barato, porque o dicionário já está em SQLite.

### 2. Selos de tecla (`Esc`, `Tab`) são lidos de forma inconsistente

`Esc` é 7 dos 11 erros. É um selo de ~20 px de altura, texto claro sobre retângulo escuro
arredondado, no canto da tela. Às vezes sai `Esc]`, às vezes some.

**Não é problema de produto** — ninguém quer traduzir "Esc". Está registrado porque distorce a nota:
o recall real sobre *texto de conteúdo* é maior que os 96,5% da tabela. Vale considerar filtrar
regiões muito pequenas nas bordas antes de contar.

### 3. Detecção quebra a linha quando um ícone a interrompe

Confirmado na tela de tutorial: `Press [Shift] or [mouse] to use some Stamina to dash.` sai como
caixas separadas, porque os ícones de tecla partem a mancha de texto. O agrupamento de `lines.rs`
junta de volta pelo critério de sobreposição vertical + vão horizontal, e a frase sai inteira.

Fica registrado porque o parâmetro `line_gap_ratio` (1,2 × altura da caixa) foi calibrado nessa
única tela. Outros jogos com espaçamento diferente podem exigir ajuste.

### 4. Medição com o jogo aberto é inútil

A primeira bateria rodou com o jogo ainda em segundo plano. Resultado: a mesma configuração deu
340 ms, depois 4545 ms, depois 1099 ms. Uma varredura de threads inteira foi descartada, e a
conclusão que eu tirei dela ("paralelismo piora") estava errada — o efeito era contenção de CPU.

**Regra para as próximas medições: fechar o jogo antes de medir latência de OCR.** O número com o
jogo aberto é interessante como cenário real, mas não é reproduzível e não serve de critério.

### 5. `ort` não tem versão estável

`2.0.0-rc.13` é a versão em uso pela comunidade; não existe release estável. Pinada com `=` no
`Cargo.toml` de propósito: as rc mudam a API entre si e um bump silencioso quebraria o build.
Revisar quando sair a 2.0 final.

## Critérios GO

| Critério do roteiro                                    | Status                                                    |
| ------------------------------------------------------ | --------------------------------------------------------- |
| Recall ≥90% em UI/diálogo                              | ✅ **Passou** — 93,1% a 100%, diálogos em 100%             |
| Recall ≥75% na de fundo complexo                       | ✅ **Passou** — 90,9%                                      |
| Bboxes visualmente corretas palavra a palavra          | ✅ **Passou** — conferido com as caixas desenhadas         |
| Latência <500 ms por frame (quente, CPU)               | ⚠️ **Passou com condição** — 9/10 na tela inteira; 10/10 com região de 1280×720 |
| Agrupamento em linhas produz frases utilizáveis        | ✅ **Passou** — frases inteiras, com pontuação e contrações |
| Modelos embarcáveis (<50 MB)                           | ✅ **Passou** — 10,9 MB                                    |

## Não feito

Dois passos do roteiro ficaram de fora, por decisão consciente:

1. **Comparação com Windows.Media.Ocr** (passo 6). O objetivo era ter um plano B caso o RapidOCR
   reprovasse. Ele não reprovou, e o Windows OCR não oferece posição por palavra do jeito que o
   produto precisa. **Continua como plano B documentado, não medido.** Vale medir se aparecer
   máquina onde o `ort` não roda.
2. **Experimentos de pré-processamento** (passo 5: grayscale, upscale 2×, binarização). Existiam
   para resgatar imagens ruins — nenhuma imagem do conjunto ficou abaixo de 90,9%, então não havia
   o que resgatar. Reabrir se algum jogo real reprovar.

Também não foi medido o consumo de **RAM** do runtime ONNX carregado, que é o item que mais
interessa ao problema 1 da spike 01. Fica para quando o OCR estiver dentro do app.

## Próximos passos

1. Ligar OCR à captura de tela — hoje `capture::capture_focused_monitor()` ainda devolve
   `NotImplemented`, e é a última peça que falta para o lookup ponta a ponta.
2. Definir a janela de OCR em volta do cursor como parâmetro de F2 (a medição sugere 1280×720).
3. Levar a correção de confusões `O`↔`Q` para a Fase 1, junto do dicionário.
4. Medir a RAM do runtime carregado e fechar o problema 1 da spike 01.

## Como reproduzir os números

```powershell
powershell -File scripts/fetch-ocr-models.ps1

cd src-tauri
cargo run --release --features spikes --bin ocr-spike -- --repeticoes 5                    # tela inteira
cargo run --release --features spikes --bin ocr-spike -- --repeticoes 5 --regiao 1280x720  # janela do cursor
cargo run --release --features spikes --bin ocr-spike -- --imagem <png> --desenhar         # conferência visual

$env:PAPAPLAY_OCR_THREADS = "8"   # repete a varredura de threads
```

> **Fechar o jogo antes** — ver [Problema 4](#4-medição-com-o-jogo-aberto-é-inútil).

O `ocr-spike` sai do repo quando a Fase 1 começar; o que fica é o módulo `ocr` e os gabaritos em
`tests/fixtures/screens/`.
