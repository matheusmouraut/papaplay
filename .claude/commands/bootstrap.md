---
description: Scaffold inicial do app Tauri 2 + React/TS + Tailwind (rodar 1x na primeira sessão)
---

Faça o scaffold inicial do PapaPlay. Leia CLAUDE.md e docs/04-arquitetura.md antes.

Passos:

1. Verifique pré-requisitos: `node -v` (>=20), `pnpm -v`, `rustc --version`, e o Microsoft C++ Build Tools/WebView2 (documente se faltar algo, com instrução de instalação).
2. Scaffold com `pnpm create tauri-app@latest . --template react-ts --manager pnpm` (ajuste flags conforme a versão atual do CLI; app name `papaplay`, identifier `com.papaplay.app`). NÃO sobrescrever CLAUDE.md, docs/, .claude/, tests/, scripts/.
3. Adicione e configure: Tailwind CSS, ESLint + Prettier, Zustand, TanStack Query, ts-fsrs.
4. Configure o Tauri para DUAS janelas em `tauri.conf.json`: `main` (app principal, visível) e `overlay` (transparent: true, decorations: false, alwaysOnTop: true, skipTaskbar: true, hidden por padrão). Adicione os plugins `global-shortcut` e `single-instance`.
5. Estruture `src/` conforme docs/04: `src/overlay/`, `src/main/`, `src/shared/`, com uma tela placeholder em cada janela provando o roteamento por janela.
6. Crie módulos vazios no core Rust: `capture.rs`, `ocr.rs`, `dict.rs`, `translate.rs`, `deck.rs`, `hotkeys.rs` com um comando Tauri `ping` de exemplo chamado pela UI.
7. `pnpm tauri dev` deve abrir a janela principal sem erros. `cargo clippy` e `pnpm lint` limpos.
8. Git: `git init` (se necessário), .gitignore adequado (node_modules, target, dist, *.db locais de teste), commit inicial `chore: bootstrap tauri app`.

Ao final, liste o que foi criado e o que ficou pendente.
