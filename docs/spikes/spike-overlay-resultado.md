# Spike 01 — Resultado: overlay transparente sobre jogo real

**Roteiro:** [`spike-01-overlay.md`](spike-01-overlay.md) · **Branch:** `spike/overlay` · **Data:** 2026-07-30

**Veredito: GO com ressalvas.** Tudo que dá para medir por linha de comando passou com folga
(alternância 15× mais rápida que a meta, CPU em repouso ~0,3% de um núcleo, posicionamento exato
num monitor secundário com DPI 150%). **Nenhum jogo foi testado** — os 3 alvos do roteiro, o FPS e
o fullscreen exclusivo exigem alguém na máquina e continuam pendentes. Ver
[Pendente de validação manual](#pendente-de-validação-manual).

---

## Máquina de teste

| Item     | Valor                                                                    |
| -------- | ------------------------------------------------------------------------ |
| SO       | Windows 11 Home Single Language, build 26200                              |
| CPU      | Intel Core i7-12700H (14C/20T)                                            |
| RAM      | 31,7 GB                                                                   |
| GPU      | Intel Iris Xe + NVIDIA RTX 3060 Laptop                                    |
| Monitor 1| 2560×1600 @ escala 150%, **secundário**, origem `-2560,0` (painel laptop) |
| Monitor 2| 2560×1440 @ escala 100%, primário, origem `0,0`                           |
| WebView2 | 150.0.4078.105                                                            |
| Build    | `pnpm tauri build --no-bundle` (release, otimizado)                       |

O setup é favorável para a spike: dois monitores, escalas de DPI diferentes e o monitor secundário
com coordenada X negativa — exatamente o caso que costuma quebrar posicionamento de overlay.

## O que foi implementado

- `src-tauri/src/overlay.rs` — modo passivo (click-through) × modo lookup (interativo),
  posicionamento sobre o monitor da janela em foco, restauração de foco na saída,
  instrumentação de latência e verificação de geometria.
- `src-tauri/src/platform.rs` — consultas Win32 read-only (`GetForegroundWindow`,
  `MonitorFromWindow`, `GetMonitorInfoW`, `GetWindowTextW`) + `SetForegroundWindow` para devolver
  o foco. **Nenhum hook, nenhuma injeção** — respeita a regra 1 do CLAUDE.md.
- `src-tauri/src/hotkeys.rs` — `Alt+X` global alterna o modo; `Esc` registrado **dinamicamente**,
  só enquanto a overlay está em lookup.
- `src/overlay/App.tsx` — retângulo de teste semi-transparente, botão clicável, marcas nos 4 cantos
  (conferência visual de cobertura do monitor) e HUD com os números ao vivo.

## Números medidos

### Alternância de modo (50 alternâncias seguidas, 0 falhas)

Medido no core Rust em volta do bloco inteiro da transição — inclui consulta da janela em foco,
reposicionamento, `set_ignore_cursor_events`, `set_focus`/`SetForegroundWindow` e o registro
dinâmico do `Esc`.

| Direção                | n   | média    | p50      | p95      | máx      |
| ---------------------- | --- | -------- | -------- | -------- | -------- |
| Entrar em lookup       | 25  | 13,1 ms  | 13,0 ms  | 17,2 ms  | 18,2 ms  |
| Voltar para passivo    | 25  | 6,5 ms   | 6,4 ms   | 8,6 ms   | 9,4 ms   |
| **Geral**              | 50  | 9,8 ms   | 10,3 ms  | 15,9 ms  | **18,2 ms** |

**Meta: <150 ms. Pior amostra: 18,2 ms — 8× abaixo do teto.** Nenhuma amostra passou de 20 ms,
nenhuma falha em 50 alternâncias consecutivas.

Entrar custa ~2× mais que sair porque só a entrada consulta a janela em foco, reposiciona a janela
e rouba o foco. Mesmo assim sobra orçamento de sobra: o critério de `<300 ms` para o tooltip
(doc 03) tem ~280 ms livres depois da alternância.

### Consumo em repouso (modo passivo, 3 janelas de 60 s)

Árvore inteira de processos: `papaplay.exe` + 7 filhos `msedgewebview2.exe`.

| Janela | CPU na janela | % de 1 núcleo | % do sistema (20T) | Working set | Private bytes |
| ------ | ------------- | ------------- | ------------------ | ----------- | ------------- |
| 1      | 203,1 ms      | 0,339 %       | 0,017 %            | 440,1 MB    | 252,0 MB      |
| 2      | 78,1 ms       | 0,130 %       | 0,007 %            | 439,4 MB    | 250,4 MB      |
| 3      | 312,5 ms      | 0,521 %       | 0,026 %            | 440,4 MB    | 249,6 MB      |

**CPU: meta ~0% atingida** — 0,13 a 0,52% de um núcleo, ou 0,007 a 0,026% da CPU total.
Na prática é ruído: ~200 ms de CPU a cada minuto, com duas WebViews vivas.

**RAM: ver [Problema 1](#1-ram-já-estoura-a-meta-antes-de-qualquer-modelo).**

### Geometria / DPI

Verificação automática (`overlay_check_geometry`): entra em lookup, lê a posição e o tamanho reais
da janela e compara com o retângulo do monitor alvo.

```json
{
  "monitor": { "x": -2560, "y": 0, "width": 2560, "height": 1600 },
  "window":  { "x": -2560, "y": 0, "width": 2560, "height": 1600 },
  "scaleFactor": 1.5,
  "matches": true
}
```

Cobertura **exata** (tolerância de 1 px) no monitor secundário, com escala de 150% e origem X
negativa. Usar pixels físicos do `MONITORINFO` direto no `set_position`/`set_size` do Tauri evita
a conversão de DPI no meio do caminho — é o que faz esse caso dar certo.

### Transparência e always-on-top

Confirmado por screenshot da área de trabalho com o app rodando: o conteúdo por baixo (navegador,
terminal) aparece inteiro através da overlay, que cobre o monitor todo; o retângulo de teste e o
HUD são desenhados por cima, inclusive por cima da janela principal do próprio PapaPlay. A ponte
UI↔Rust também respondeu (`Core Rust: pong v0.1.0`).

Isso valida transparência + always-on-top **contra janelas normais do Windows**. Contra um jogo em
borderless, não — ver pendências.

## Problemas encontrados

### 1. RAM já estoura a meta antes de qualquer modelo

440 MB de working set / 250 MB de private bytes, com **8 processos** e **zero modelos carregados**.
O requisito não-funcional do doc 04 é "<400 MB com modelos carregados (lazy-load do NMT)".

A conta não fecha: duas janelas WebView2 (overlay + principal) já custam isso sozinhas, e ainda
faltam ONNX Runtime + RapidOCR (~20 MB de modelos, mas o runtime aloca bem mais) e o Marian/Bergamot
(~40 MB por par). Não é bloqueio da spike de overlay — a alternância e a CPU passaram — mas é um
risco real para o alvo de RAM do MVP.

Caminhos a avaliar (fora do escopo desta spike):

- Não manter a janela principal viva o tempo todo — criar sob demanda e destruir ao fechar.
- Conferir se `private bytes` (250 MB) é a métrica honesta aqui; boa parte do working set é página
  compartilhada do runtime WebView2, que o Windows contabiliza em todo processo que o usa.
- Revisar a meta de 400 MB no doc 04 com número medido, em vez de estimado.

### 2. Roubo de foco ao entrar em lookup

Entrar em lookup chama `set_focus()` — o jogo perde o foco. Na saída, `SetForegroundWindow` devolve
para a janela guardada, mas a API é *best-effort*: o Windows recusa a chamada quando o processo não
está em primeiro plano, e nesse caso a função retorna `FALSE` silenciosamente.

Em janelas normais funcionou. Em jogo borderless em tela cheia, perder o foco pode minimizar o jogo
em alguns títulos — **é o principal risco que a validação manual precisa responder.**

### 3. `Esc` global é registrado dinamicamente (de propósito)

Um `Esc` global permanente seria roubado do jogo: o menu de pausa pararia de abrir. Por isso o
atalho só existe enquanto a overlay está em lookup, e o registro/cancelamento entra no caminho
crítico da alternância (já contabilizado nos 13 ms de entrada).

Efeito colateral aceito: durante o lookup, o `Esc` não chega ao jogo. É o comportamento desejado
(`Esc` fecha o lookup), mas vale registrar.

### 4. Armadilha de build que quase invalidou a medição

`cargo build --release` **não embute o frontend** — o binário resultante aponta para `localhost:1420`
e a WebView carrega a página de erro `ERR_CONNECTION_REFUSED`. A primeira rodada de CPU/RAM foi
medida contra essa página de erro e teve que ser descartada; o screenshot foi o que pegou o
problema. Medição de release tem que sair de `pnpm tauri build`.

Para referência, o quanto isso distorceu: 355 MB de working set com a página de erro contra
440 MB com a UI real — 85 MB, ~19% de diferença.

### 5. A overlay fixa o monitor no momento da alternância

O monitor alvo é resolvido quando a hotkey é apertada. Se o usuário troca de tela sem sair do modo
lookup, a overlay fica no monitor antigo até a próxima alternância. Aceitável para o MVP.

### 6. Gancho de instrumentação a remover

`PAPAPLAY_SPIKE_BENCH` / `PAPAPLAY_SPIKE_BENCH_OUT` (em `overlay.rs`) rodam o benchmark no boot e
gravam JSON. Existem para o número ser reproduzível por linha de comando; **saem junto com a spike.**

## Critérios GO

| Critério do roteiro                                       | Status                                                                 |
| --------------------------------------------------------- | ---------------------------------------------------------------------- |
| Overlay visível e clicável sobre os 3 alvos em borderless | ⏳ **Não testado** — validado só contra janelas normais do Windows      |
| Click-through: em modo passivo o jogo recebe os cliques   | ⏳ **Não testado** — `set_ignore_cursor_events` aplicado sem erro       |
| Alternância <150 ms, 50 seguidas sem travar               | ✅ **Passou** — máx 18,2 ms, 0 falhas em 50                            |
| Sem queda de FPS perceptível (>5%) com overlay passivo    | ⏳ **Não testado** — CPU em repouso é 0,3% de um núcleo, o que ajuda    |
| Hotkey global funciona com o jogo em foco                 | ⏳ **Não testado** — `Alt+X` registra e dispara no desktop              |
| _(extra)_ Cobertura exata do monitor com DPI ≠ 100%       | ✅ **Passou** — match exato em monitor secundário @150%                 |
| _(extra)_ Transparência + always-on-top                   | ✅ **Passou** — confirmado por screenshot                              |

## Pendente de validação manual

Nada disso dá para automatizar daqui: precisa de um jogo rodando e de alguém olhando a tela.
Com o app buildado (`pnpm tauri build --no-bundle`) e rodando, para cada alvo:

1. **Jogo 3D em borderless** — abrir o jogo, apertar `Alt+X`.
   - A overlay aparece por cima do jogo?
   - O retângulo de teste deixa o jogo visível por baixo?
   - As 4 marcas de canto encostam nas bordas da tela? (se não, o monitor errado foi coberto)
   - O botão "Cliquei N×" responde ao clique?
   - `Esc` volta para passivo e **o jogo recupera o foco** (problema 2)?
   - Em modo passivo, clicar em cima do retângulo: o clique chega ao jogo?
2. **Jogo 2D/indie** — mesma sequência.
3. **Vídeo em tela cheia no player local** — mesma sequência.
4. **FPS** — com contador do Steam ou RivaTuner, medir o FPS médio do jogo por ~2 min sem o
   PapaPlay e ~2 min com ele em modo passivo. Critério: queda <5%.
5. **Fullscreen exclusivo** — abrir um jogo em fullscreen exclusivo e apertar `Alt+X`. O esperado
   é a overlay **não** aparecer; documentar o que acontece de fato (a tela pisca? o jogo minimiza?).

O HUD já mostra na tela o título da janela em foco, o retângulo do monitor coberto, a escala de DPI
e a latência da última alternância — é só ler e anotar. O botão "Benchmark: 50 alternâncias" repete
a medição de latência com o jogo em foco, que é o número que realmente importa.

## Próximos passos

1. **Rodar a validação manual acima** com um jogo real. É o que fecha o veredito.
2. Se passar: remover o gancho `PAPAPLAY_SPIKE_BENCH`, enxugar a UI de spike do
   `src/overlay/App.tsx` (o HUD vira a base do popup de lookup) e mergear `spike/overlay`.
3. Levar o achado de RAM (problema 1) para o doc 04 — a meta de 400 MB precisa de número medido.
4. Seguir para a [spike 02 (OCR)](spike-02-ocr.md), que é o outro gate da Fase 0.

## Plano B (se a validação manual reprovar)

Do doc 04, na ordem de menor para maior custo:

- **Falha só de foco** (jogo minimiza ao entrar em lookup): tentar `WS_EX_NOACTIVATE` na janela
  overlay e trabalhar sem `set_focus()`, encaminhando o teclado por hotkeys globais. Mantém a stack.
- **Falha de composição** (overlay não aparece sobre o jogo): janela nativa Win32 via `windows-rs`
  com WebView só para o popup de lookup.
- **Falha de performance** (queda de FPS): overlay em `egui`/`wgpu`, sem WebView na camada que fica
  por cima do jogo.

## Como reproduzir os números

```powershell
pnpm tauri build --no-bundle

# Latência de alternância + verificação de geometria
$env:PAPAPLAY_SPIKE_BENCH = "50"
$env:PAPAPLAY_SPIKE_BENCH_OUT = "$PWD\bench.json"
.\src-tauri\target\release\papaplay.exe

# CPU/RAM em repouso: subir sem as variáveis acima e amostrar
# TotalProcessorTime da árvore papaplay.exe + msedgewebview2.exe
```
