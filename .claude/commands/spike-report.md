---
description: Executa/atualiza o relatório de uma spike da Fase 0
---

Argumento: nome da spike (ex.: `overlay` ou `ocr`). $ARGUMENTS

1. Leia o roteiro em `docs/spikes/spike-*-$ARGUMENTS.md`.
2. Execute os passos pendentes do roteiro (código em branch `spike/$ARGUMENTS`).
3. Meça o que o roteiro pede (latência, CPU, acurácia) — números reais, não estimativas.
4. Escreva/atualize `docs/spikes/spike-$ARGUMENTS-resultado.md` com: o que funcionou, números medidos, problemas encontrados, veredito (GO / NO-GO / GO com ressalvas) e próximos passos.
5. Se NO-GO, liste as alternativas do plano B documentadas em docs/04-arquitetura.md.
