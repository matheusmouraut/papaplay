# Spike 01 — Overlay transparente sobre jogo real

**Pergunta a responder:** o Tauri 2 consegue manter uma janela transparente always-on-top sobre um jogo borderless, alternando entre click-through e interativa via hotkey global, sem prejudicar o jogo?

**Branch:** `spike/overlay` · **Timebox:** 1–2 dias · **Pré-requisito:** `/bootstrap` executado.

## Roteiro

1. Na janela `overlay`: fundo transparente, um retângulo semi-transparente de teste e um botão clicável.
2. Implementar `set_ignore_cursor_events(true/false)` alternado por hotkey global `Alt+X` (plugin global-shortcut). `Esc` sempre volta para click-through.
3. Posicionar o overlay cobrindo o monitor da janela em foco no momento da hotkey (multi-monitor: testar com 2 monitores se disponível).
4. Testar sobre 3 alvos: (a) um jogo 3D real em borderless (ex.: qualquer jogo da sua biblioteca), (b) um jogo 2D/indie, (c) um vídeo fullscreen no player local.
5. Medir: CPU/RAM do processo em modo passivo (meta ~0%), tempo de alternância do modo (meta <150ms), FPS do jogo com e sem overlay (RivaTuner/Steam FPS counter).
6. Testar comportamento com jogo em fullscreen exclusivo — documentar o que acontece (esperado: overlay não aparece; confirmar).

## Critérios GO

- [ ] Overlay visível e clicável sobre os 3 alvos em borderless
- [ ] Click-through funciona: com modo passivo, o jogo recebe todos os cliques
- [ ] Alternância <150ms e confiável (50 alternâncias seguidas sem travar)
- [ ] Sem queda de FPS perceptível (>5%) no jogo com overlay passivo
- [ ] Hotkey global funciona com o jogo em foco

## Registrar em `spike-overlay-resultado.md`

Números medidos, jogos testados, problemas (ex.: DPI scaling, multi-monitor), veredito GO/NO-GO. Plano B se NO-GO: janela nativa Win32 via windows-rs com WebView só para o popup, ou egui/wgpu para o overlay.
