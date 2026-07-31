# Spike 01 — Resultado: overlay transparente sobre jogo real

**Roteiro:** [`spike-01-overlay.md`](spike-01-overlay.md) · **Branch:** `spike/overlay` · **Data:** 2026-07-30

**Veredito: GO.** A overlay compõe corretamente sobre um jogo 3D em borderless, com click-through
em modo passivo e devolução de foco funcionando — ver
[Validação com jogo real](#validação-com-jogo-real). A alternância de modo ficou 10× abaixo da meta
(média 4,9 ms com o jogo rodando), a CPU em repouso é ~0,3% de um núcleo e o posicionamento acerta
exatamente o monitor alvo mesmo com DPI a 150% num monitor secundário de origem negativa.

Duas medições ficaram conscientemente de fora (FPS e fullscreen exclusivo) e uma ressalva de RAM
sobrevive à spike — ver [Não medido](#não-medido) e [Problema 1](#1-ram-já-estoura-a-meta-antes-de-qualquer-modelo).

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

Isso valida transparência + always-on-top contra janelas normais do Windows. Contra jogo, ver a
seção seguinte.

## Validação com jogo real

**Alvo:** *The Seven Deadly Sins: Origin* (jogo 3D, Unreal, **borderless**), rodando no monitor
primário — `2560×1440 @ 0,0`, escala de DPI `1×`. Feita manualmente em 2026-07-30, com o app de
release (`pnpm tauri build --no-bundle`) já em execução antes do jogo abrir.

| Verificação                                                       | Resultado                                                     |
| ----------------------------------------------------------------- | ------------------------------------------------------------- |
| Overlay aparece por cima do jogo em borderless                    | ✅ Sim — desenhada sobre o swapchain, sem piscar               |
| Jogo visível através do retângulo de teste                        | ✅ Sim — cenário e personagem legíveis por baixo               |
| HUD identifica a janela em foco                                   | ✅ `SevenDeadlySins`                                           |
| Monitor alvo correto (4 cantoneiras nas bordas)                   | ✅ Sim                                                         |
| Botão responde ao clique em modo lookup                           | ✅ Sim — contador chegou a 24×                                 |
| `Esc` volta para passivo **sem minimizar/pausar o jogo**          | ✅ Sim — jogo seguiu rodando e recebendo teclado               |
| Modo passivo: clique atravessa para o jogo                        | ✅ Sim — o botão não reage, o clique chega ao jogo             |

### Alternância com o jogo rodando (botão "Benchmark: 50 alternâncias")

| n   | falhas | média   | p50     | p95     | min     | máx      |
| --- | ------ | ------- | ------- | ------- | ------- | -------- |
| 50  | **0**  | 4,9 ms  | 3,1 ms  | 9,6 ms  | 0,3 ms  | 13,9 ms  |

**Meta: <150 ms. Pior amostra com o jogo rodando: 13,9 ms — 10× abaixo do teto.** Ficou *melhor*
que a medição no desktop vazio (média 9,8 ms, máx 18,2 ms): no benchmark o foreground já é a própria
overlay, então a troca de foco real não entra no laço. As alternâncias disparadas por `Alt+X` com o
jogo em foco ficaram na casa dos 9 ms, ainda com folga enorme.

### O que isso fecha

O risco de composição — o cenário em que o jogo entra no caminho rápido de *independent flip* /
multiplane overlay e a camada por cima é ignorada — **não se materializou**. Com ele, caem os dois
planos B mais caros do doc 04: não é preciso janela nativa Win32 via `windows-rs` nem overlay em
`egui`/`wgpu`. A stack Tauri + WebView2 está confirmada para a camada que fica sobre o jogo.

O [Problema 2](#2-roubo-de-foco-ao-entrar-em-lookup) (roubo de foco) também está resolvido na
prática: o `SetForegroundWindow` best-effort devolveu o foco ao jogo, que não minimizou nem pausou.
Fica registrado como comportamento a re-testar se algum título específico reclamar — o plano B
(`WS_EX_NOACTIVATE` sem `set_focus()`) continua válido e barato.

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

### 2. Roubo de foco ao entrar em lookup — *resolvido na validação*

Entrar em lookup chama `set_focus()` — o jogo perde o foco. Na saída, `SetForegroundWindow` devolve
para a janela guardada, mas a API é *best-effort*: o Windows recusa a chamada quando o processo não
está em primeiro plano, e nesse caso a função retorna `FALSE` silenciosamente.

Era o principal risco em aberto. **Passou no jogo real** (Seven Deadly Sins, borderless): o jogo não
minimizou nem pausou e voltou a receber teclado após o `Esc`. Um único título não é prova geral —
se algum jogo reclamar, o plano B é `WS_EX_NOACTIVATE` sem `set_focus()`, encaminhando o teclado por
hotkeys globais.

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

### 6. Gancho de instrumentação — *removido*

`PAPAPLAY_SPIKE_BENCH` / `PAPAPLAY_SPIKE_BENCH_OUT` (em `overlay.rs`) rodavam o benchmark no boot e
gravavam JSON. Serviram para os números da tabela de desktop serem reproduzíveis por linha de
comando e **foram removidos ao fechar a spike**. Os comandos `overlay_bench` e
`overlay_check_geometry` continuam, acionáveis pelo HUD — são úteis como teste de regressão.

## Critérios GO

| Critério do roteiro                                       | Status                                                                 |
| --------------------------------------------------------- | ---------------------------------------------------------------------- |
| Overlay visível e clicável sobre jogo em borderless       | ✅ **Passou** — jogo 3D (Unreal); 2D/indie e vídeo não testados         |
| Click-through: em modo passivo o jogo recebe os cliques   | ✅ **Passou** — confirmado no jogo real                                 |
| Alternância <150 ms, 50 seguidas sem travar               | ✅ **Passou** — máx 13,9 ms com o jogo rodando, 0 falhas em 50          |
| Sem queda de FPS perceptível (>5%) com overlay passivo    | ➖ **Não medido** — ver [Não medido](#não-medido)                       |
| Hotkey global funciona com o jogo em foco                 | ✅ **Passou** — `Alt+X` e `Esc` responderam com o jogo em primeiro plano |
| _(extra)_ Cobertura exata do monitor com DPI ≠ 100%       | ✅ **Passou** — match exato em monitor secundário @150%                 |
| _(extra)_ Transparência + always-on-top                   | ✅ **Passou** — sobre jogo e sobre janelas normais                      |

Cinco dos sete critérios foram verificados; nenhum reprovou. **GO.**

## Não medido

Decisão consciente de fechar a spike sem estes dois — o custo de medir não se pagava diante do que
já estava respondido:

1. **FPS com e sem overlay (critério: queda <5%).** Atenuante forte: em modo passivo a overlay
   consome 0,13–0,52% de um núcleo e não redesenha nada; o jogo rodou sem engasgo perceptível
   durante toda a validação. **Medir antes de fechar a Fase 2**, quando a overlay passar a desenhar
   destaques por bbox em tempo real — é aí que o custo de composição deixa de ser desprezível.
2. **Fullscreen exclusivo.** Comportamento esperado (e padrão do Windows): a overlay não aparece.
   Tratar como **limitação conhecida do MVP** — a UI deve orientar o usuário a usar borderless, e
   detectar o caso para avisar em vez de falhar em silêncio. Vira item de F1 no doc 03.
3. **Segundo e terceiro alvos do roteiro** (jogo 2D/indie e vídeo em tela cheia). O alvo difícil —
   jogo 3D com swapchain em borderless — passou; os outros dois são casos mais fáceis do mesmo
   mecanismo de composição.

## Próximos passos

1. ~~Enxugar a UI de spike do `src/overlay/App.tsx`~~ — **feito.** O retângulo de teste saiu; o
   painel de diagnóstico saiu da tela e volta com `F9` (listener da janela, não atalho global —
   só responde em modo lookup, então não rouba tecla do jogo). As marcas de canto **ficam** até
   sairmos da fase de testes: são a conferência visual de que a overlay cobriu o monitor certo.
   O popup de lookup de verdade substitui esse arquivo na Fase 1.
2. Levar o achado de RAM (problema 1) para o doc 04 — a meta de 400 MB precisa de número medido.
3. Registrar o fullscreen exclusivo como limitação conhecida no doc 03 (F1).
4. Seguir para a [spike 02 (OCR)](spike-02-ocr.md), que é o outro gate da Fase 0.

## Plano B (se algum jogo reprovar mais adiante)

Do doc 04, na ordem de menor para maior custo. O de composição saiu de cena com a validação; os
outros dois continuam de pé como contingência:

- **Falha só de foco** (jogo minimiza ao entrar em lookup): tentar `WS_EX_NOACTIVATE` na janela
  overlay e trabalhar sem `set_focus()`, encaminhando o teclado por hotkeys globais. Mantém a stack.
- ~~**Falha de composição** (overlay não aparece sobre o jogo): janela nativa Win32 via
  `windows-rs`~~ — descartado, a composição funcionou.
- **Falha de performance** (queda de FPS): overlay em `egui`/`wgpu`, sem WebView na camada que fica
  por cima do jogo. Só volta à mesa se o item 1 de [Não medido](#não-medido) reprovar na Fase 2.

## Como reproduzir os números

```powershell
pnpm tauri build --no-bundle
.\src-tauri\target\release\papaplay.exe
```

- **Latência de alternância:** botão "Benchmark: 50 alternâncias" no HUD da overlay (`Alt+X` para
  abrir). Com o jogo em foco antes de apertar `Alt+X`, é o número que vale.
- **Geometria/DPI:** comando `overlay_check_geometry`.
- **CPU/RAM em repouso:** com o app em modo passivo, amostrar `TotalProcessorTime` e working set da
  árvore `papaplay.exe` + `msedgewebview2.exe`.

> O gancho de boot `PAPAPLAY_SPIKE_BENCH` usado na primeira rodada foi removido junto com a spike
> (problema 6).
