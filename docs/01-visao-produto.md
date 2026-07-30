# 01 — Visão do Produto

## O problema

Quem quer aprender inglês tem duas vidas separadas: a vida de estudo (apps de curso, Anki, aulas) e a vida real (jogos, séries, internet). A ponte entre as duas é fraca: você encontra uma palavra desconhecida jogando, alt-tab para o Google Tradutor, perde a imersão e nunca mais revisa aquela palavra. O aprendizado por imersão funciona, mas a fricção mata o hábito.

## A tese

**Transformar o conteúdo que você já consome em material de estudo, sem sair dele.** O jogo é a aula. A palavra desconhecida vira card. A revisão acontece depois, no ritmo certo (repetição espaçada). O usuário não muda de comportamento — ele só ganha uma camada de aprendizado sobre o que já faz.

## Personas

**Persona principal — o Gamer aprendiz (você):** joga regularmente no PC, nível básico/intermediário de inglês, já tentou apps tradicionais e abandonou por tédio. Quer jogar em inglês mas trava em diálogos e menus. Toparia revisar 10 min/dia se os cards viessem dos jogos dele.

**Persona secundária — o Maratonista de séries:** assiste com legenda em inglês, pausa para traduzir no celular. (Atendido na Fase 2 do roadmap.)

## Concorrentes e referências

| Ferramenta                                                                                                                                                               | O que faz                                                                                    | Limitação/gap                                                                                                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| [Lookupper](https://lookupper.com/)                                                                                                                                      | Lookup hover+hotkey em qualquer app, dicionário embutido, 29 idiomas, offline, MS Store 4.6★ | Melhor UX de lookup do mercado, mas é só dicionário de tela: sem deck rico, sem SRS, sem memória do que o usuário sabe. Ver doc 07 |
| [Playto](https://playto.dev/)                                                                                                                                            | Overlay OCR + SRS para jogos, 12 idiomas, offline                                            | Concorrente mais direto. Foco em tradução de tela inteira; UX de lookup palavra-a-palavra menos central; sem foco PT-BR            |
| [GameSentenceMiner](https://github.com/bpwhelan/GameSentenceMiner)                                                                                                       | Mineração de sentenças de jogos → Anki                                                       | Focado em japonês, setup técnico complexo (Anki, Yomitan, text hooks), público hardcore                                            |
| [LunaTranslator](https://docs.lunatranslator.org/en/)                                                                                                                    | Tradutor de visual novels via text hooking                                                   | Nicho de visual novels, UX datada, curva de aprendizado alta                                                                       |
| [Game Overlay Translator](https://store.steampowered.com/app/4864520/Game_Overlay_Translator/) / [RSTGameTranslation](https://github.com/thanhkeke97/RSTGameTranslation) | Tradução de tela em tempo real                                                               | Só traduz — não ensina. Sem deck, sem revisão, sem progresso                                                                       |
| Language Reactor / Migaku                                                                                                                                                | Legendas interativas em streaming                                                            | Só navegador/streaming; não cobre jogos                                                                                            |

**Leitura do mercado:** existem "tradutores de tela" (não ensinam) e "ferramentas de estudo" (não integram com o conteúdo). O Playto valida que o espaço existe. Nosso jogo é vencer em **experiência**: lookup instantâneo palavra-a-palavra, zero configuração, e o melhor fluxo captura→revisão para brasileiros.

## Diferenciais do nosso produto

1. **Lookup palavra-a-palavra como interação primária** — hover/clique numa palavra específica, como o Yomitan faz em páginas web, mas em qualquer jogo. Concorrentes traduzem blocos/tela inteira.
2. **Foco EN→PT-BR bem feito** — dicionário com traduções naturais, notas de gírias/phrasal verbs comuns em jogos, em vez de suporte raso a 20 idiomas.
3. **Zero fricção** — instalar, abrir o jogo, apertar hotkey. Sem Anki, sem plugins, sem configuração por jogo.
4. **100% local e gratuito de operar** — OCR, dicionário e tradução rodam offline. Sem assinatura para o núcleo.
5. **Plataforma, não ferramenta** — a mesma base (captura → lookup → deck → revisão) se estende depois para séries, navegador e PDFs. Concorrentes são mono-cenário.

## Princípios de produto

- **A imersão é sagrada:** nenhuma interação pode tirar o foco do jogo por mais de ~3 segundos.
- **Capturar é mais importante que traduzir:** o valor de longo prazo está no deck pessoal, não na tradução pontual.
- **Offline-first:** nenhuma funcionalidade do núcleo depende de internet ou API paga.
- **O app se adapta ao usuário:** frequência de palavra + histórico definem o que mostrar (não mostrar tradução de palavras que ele já domina, no futuro).

## Nome

Provisório: **PapaPlay**. Alternativas: **Wordlay** (word + overlay/play), **Lexi** , **Fluenzy**, **Glossa**. Decidir antes do release público (verificar domínio/marca).

## Métrica norte

**Palavras revisadas por semana por usuário ativo.** Captura sem revisão é tradução descartável; revisão recorrente é aprendizado. Secundárias: sessões de jogo com overlay ativo/semana, retenção D30, tamanho do deck ativo.
