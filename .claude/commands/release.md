---
description: Checklist de release — versão, changelog, build, smoke test
---

1. Working tree limpo e testes verdes: `pnpm test`, `cargo test`, `pnpm lint`, `cargo clippy`.
2. Rode `/screen-test` — sem regressões.
3. Bump de versão (package.json + tauri.conf.json + Cargo.toml, mesmos números) conforme semver; pergunte se major/minor/patch se ambíguo.
4. Atualize CHANGELOG.md desde a última tag.
5. `pnpm tauri build` — instalador sem erros; anote o tamanho (meta <300MB).
6. Smoke test manual (peça ao usuário): instalar, onboarding, lookup no playground, salvar card, uma revisão.
7. Tag git `vX.Y.Z` e instruções para publicar no GitHub Releases.
