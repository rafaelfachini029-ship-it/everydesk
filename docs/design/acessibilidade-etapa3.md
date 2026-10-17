# Auditoria de acessibilidade: Etapa 3 (WCAG 2.1 AA)

**Escopo:** UI Svelte 5 em `apps/everydesk/ui` no commit `e3aa8d920`: `src/design/*`, `src/lib/ui/*`, Home (`routes/Home.svelte`, `lib/home/*`), Conectando (`routes/Connecting.svelte`, `lib/connecting/*`), sessão (`lib/RemoteCanvas.svelte`, `lib/session/*`), cartões (`IncomingCard`, `ReconfirmCard`, `lib/cards/*`), `lib/host/ConnectedBadge.svelte` e `routes/Gallery.svelte`.
**Data:** 2026-10-17. **Tipo:** só auditoria. Nenhum código foi alterado.
**Caso especial:** o conteúdo do canvas da sessão é a tela remota, e o encaminhamento do teclado para ela é intencional. Avaliei a pílula, os overlays e os cartões, não os pixels remotos.

## Resumo

| Severidade | Qtde |
|---|---|
| Crítico | 2 |
| Importante | 6 |
| Menor | 15 |

Todos os caminhos de arquivo abaixo são relativos a `apps/everydesk/ui/src/`.

---

## Crítico

### C1. Pedido de acesso e reconfirmação chegam em silêncio, com prazo de 60 s
- **Onde:** `lib/IncomingCard.svelte:199-233` (a pilha) e `lib/cards/RequestCard.svelte:40-46` (`<section>` sem `role`/live).
- **WCAG:** 4.1.3 Mensagens de status; 2.2.1 Tempo ajustável.
- **Falha:** quando aparece um `AccessCard` ou um `ReconfirmCard`, nada é anunciado. Não há live region nem `role="alertdialog"`, e o foco não muda. Quem usa leitor de tela no lado acessado não fica sabendo do pedido. O cartão expira em 60 s (ou em `ring_secs`) e a conexão é recusada ou encerrada. Isso bloqueia o fluxo principal do lado acessado. Pelo teclado, o cartão só é alcançado depois de todos os pontos de Tab da tela (ele fica no fim do DOM).
- **Correção sugerida:** colocar uma live region `role="status"`/`aria-live="assertive"` sempre montada em `IncomingCard` (montada antes da chegada do cartão), anunciando "{nome} quer ver e controlar a sua tela. Aceitar ou recusar, {n} segundos." e, no ring, "{nome} continua conectado. Manter?". Dar ao cartão `role="alertdialog"` com `aria-labelledby`/`aria-describedby` (já existem). Oferecer um atalho global para levar o foco ao cartão mais antigo (por exemplo, focar "Recusar" no primeiro cartão quando a janela tem foco e nenhum campo está sendo editado), sem roubar o foco no meio da digitação. Sobre 2.2.1: documentar que o limite de 60 s é essencial (segurança). Se não for, anunciar de novo aos 20 s.

### C2. Mensagem no meio da sessão (senha, aviso) fica fora de alcance, e as teclas digitadas vão para a máquina remota
- **Onde:** `lib/RemoteCanvas.svelte:397-410` (overlay), `:280-299` (gate do encaminhamento de teclas), `:403` (campo de senha).
- **WCAG:** 2.1.1 Teclado; 2.4.3 Ordem do foco; 4.1.3 Mensagens de status; 3.3.2 Rótulos.
- **Falha (verificada no navegador):** quando chega `session:msgbox` depois da revelação, o overlay aparece, mas o foco continua no canvas. O overlay não tem `role` nem live region. Tab é encaminhado para a máquina remota. Com `input-password`, o que a pessoa digita achando que está no campo de senha vai como teclas para a máquina remota. O único caminho pelo teclado é Alt (chamar a pílula) e mais 9 Tabs. O campo de senha só tem placeholder e `aria-label`, sem rótulo visível.
- **Correção sugerida:** transformar o overlay em `role="alertdialog"` (ou `role="dialog"` com `aria-modal="true"`) com `aria-labelledby` no título. Ao abrir, focar o campo de senha ou o botão OK, e prender o Tab ali dentro (o gate já não encaminha nada fora do canvas). Escape fecha os avisos e devolve o foco ao canvas. Usar o componente `Field` com rótulo visível "Senha".

---

## Importante

### I1. Cartão de destino no Conectando a 55 % de opacidade reprova em contraste
- **Onde:** `routes/Connecting.svelte:253-262` (`.dest { opacity: 0.55 }`), com `.code` em `:238-244`.
- **WCAG:** 1.4.3 Contraste (mínimo).
- **Falha (axe):** código do destino 2,65:1 no escuro e 2,63:1 no claro (15 px, mínimo 4,5). Nome do destino 4,41:1 no claro (22 px normal, mínimo 4,5). O estado dura todo o "Procurando" e continua no erro, porque o alcance fica congelado.
- **Correção sugerida:** esmaecer só a moldura e o ponto (por exemplo, `border-style: dashed` ou a borda `--line`), e não o texto. Ou aplicar a opacidade só em um pseudo-elemento de fundo. O texto fica com `--text`/`--text-muted` sem opacidade.

### I2. A borda do campo de texto quase não aparece
- **Onde:** `lib/ui/Field.svelte:76` (`border: 1px solid var(--line-strong)`), o mesmo em `lib/PolicySelect.svelte:137` e em `lib/RemoteCanvas.svelte:463`.
- **WCAG:** 1.4.11 Contraste de elementos não textuais.
- **Falha:** `--line-strong` contra `--surface` dá 1,89:1 no escuro e 1,73:1 no claro. O preenchimento do campo (`--surface`) contra `--bg` fica por volta de 1,1:1. O limite do campo "Para onde você quer ir?" fica abaixo de 3:1 nos dois temas.
- **Correção sugerida:** um token de borda de controle com ≥3:1 (por exemplo, a própria `--text-muted`, que dá 6,4/7,6:1, ou uma `--line-control` dedicada). Manter `--line-strong` para divisórias decorativas.

### I3. O erro do campo "Para onde você quer ir?" não é anunciado
- **Onde:** `lib/ui/Field.svelte:62`, disparado por `lib/home/GoField.svelte:38`.
- **WCAG:** 4.1.3 Mensagens de status (3.3.1 passa: o erro está escrito em texto).
- **Falha (verificada):** "Conectar" com o campo vazio mostra "Digite o código da outra máquina." no `<p class="note">`, sem `role`/live, e o foco fica no botão. O leitor de tela não fala nada.
- **Correção sugerida:** renderizar a nota sempre (vazia) com `aria-live="polite"`, ou `role="alert"` quando `error` estiver definido. Opcionalmente, devolver o foco ao campo quando houver erro de validação.

### I4. Escape fecha os popovers e o foco se perde
- **Onde:** `lib/home/SettingsPanel.svelte:25` (Escape na janela), `:28` (botão sem `aria-expanded`), `:30` (painel `role="dialog"`, foco não entra). `lib/home/ContactCards.svelte:60` (Escape fecha o menu "…").
- **WCAG:** 2.4.3 Ordem do foco; 4.1.2 Nome, função, valor.
- **Falha (verificada):** abrir Configurações e apertar Escape deixa o foco em `BODY`. Abrir o menu do contato, entrar no select e apertar Escape também deixa o foco em `BODY`. O botão de engrenagem não informa se está aberto ou fechado.
- **Correção sugerida:** ao fechar com Escape, chamar `.focus()` no botão que abriu (o "…" do contato ou a engrenagem). Adicionar `aria-expanded={open}` (e `aria-controls`) no botão de Configurações. Ou tratar o painel como disclosure (sem `role="dialog"`), ou mover o foco para o primeiro controle ao abrir.

### I5. O canvas escondido recebe foco durante o Conectando
- **Onde:** `lib/RemoteCanvas.svelte:376-384` (`tabindex="0"` sempre), montado por baixo do Conectando (`App.svelte:76-86`).
- **WCAG:** 2.4.7 Foco visível; 2.4.3 Ordem do foco.
- **Falha (verificada):** em `#busca`, a sequência de Tab é Cancelar → `canvas` (recortado em `circle(0%)`, invisível, `outline: none`) → BODY. O foco cai em um elemento que não aparece.
- **Correção sugerida:** `tabindex={revealed ? 0 : -1}` ou `inert={!revealed}` no `.remote` até a revelação. O `canvas.focus()` em `reveal()` continua funcionando.

### I6. Com 200 % de zoom, a pílula corta "Encerrar"
- **Onde:** `lib/ui/Pill.svelte:142-151` (`.tools-inner { overflow: hidden }`), `lib/session/SessionPill.svelte:233` e `:262` (`max-width`).
- **WCAG:** 1.4.10 Reflow; 1.4.4 Redimensionar texto.
- **Falha (verificada):** em 640 px de largura (1280 px com 200 %), a pílula expandida mostra só a borda vermelha de "Encerrar". O botão fica cortado, recebe foco, mas não aparece. Em 320 px, quase todas as ferramentas ficam cortadas.
- **Correção sugerida:** em larguras estreitas, deixar a pílula quebrar em duas linhas (`flex-wrap`) ou recolher as ferramentas "em breve" num "Mais…". "Encerrar" deve ficar sempre visível, por exemplo fixo à direita do resumo.

---

## Menor

| # | Onde | WCAG | Falha | Correção sugerida |
|---|---|---|---|---|
| M1 | `lib/home/BigCode.svelte:23`; `routes/Home.svelte:84` | 1.3.1 / 2.4.6 | A Home não tem `<h1>` (axe `page-has-heading-one` nos dois temas, também em `#conectado`). | Transformar "Seu código" em `<h1>` (mesmo visual), ou incluir um `<h1 class="sr-only">EveryDesk</h1>`. |
| M2 | `lib/home/BigCode.svelte:99-103` | 1.4.3 | Os hífens do código (`.sep`, muted × 0,55) ficam em 2,74:1 no escuro e 2,50:1 no claro. É texto grande, então o mínimo é 3:1. | Opacidade ≥0,65, ou `--text-muted` sem opacidade. |
| M3 | `routes/Connecting.svelte:238-244`, `:376-379`; `lib/cards/RequestCard.svelte:131-140`; `lib/session/SessionPill.svelte:282-285`, `:346-350`, `:359-368`; `lib/host/ConnectedBadge.svelte:59`, `:91`; `lib/home/SettingsPanel.svelte:95-100`; `lib/cards/AccessCard.svelte:102` | Regra do projeto (texto cinza pequeno) | Texto `--text-muted` em `--fs-label` (15 px), o que vai contra "texto muted usa `--fs-body` ou maior" (`typography.css:5`). O contraste passa (≥5,8:1). | Subir para `--fs-body`, ou usar `--text`. |
| M4 | `lib/home/BigCode.svelte:31`; `lib/session/SessionPill.svelte:143`, `:195` | 4.1.2 | `aria-label` em `<p>`/`<span>` genéricos ("carregando seu código", "Atraso …", "… por segundo") é ignorado pelos leitores de tela. | Texto escondido visualmente (`.sr-only`) ao lado do número, ou `role="img"`/`role="status"`. |
| M5 | `lib/RemoteCanvas.svelte:376-384` | 4.1.2 / 3.3.2 | O canvas focável não tem função nem nome, e nada diz que Alt chama os controles. | `role="application"`, `aria-roledescription="tela remota"`, `aria-label="Tela de {nome}"` e `aria-describedby` → "Toque Alt para os controles da sessão." |
| M6 | `lib/ui/Pill.svelte:72-91`; `lib/session/SessionPill.svelte:215` | 2.4.3 / 4.1.2 | Tab depois de "Encerrar" cai em `BODY`, que encaminha as teclas para a máquina remota, sem foco visível (verificado). `role="toolbar"` sem navegação por setas. | Ao sair pelo último controle, devolver o foco ao canvas (`onreturn`) ou fazer o Tab voltar ao primeiro. Adicionar setas ←/→ com roving tabindex, ou trocar para `role="group"`. |
| M7 | `lib/home/ContactCards.svelte:170-176`; `lib/home/BigCode.svelte:114-118`; `lib/session/SessionPill.svelte:286-291`; `routes/Connecting.svelte:264-274` | 1.4.11 | Pontos vazados (offline, voz desligada, destino ainda não chegou) com borda `--line-strong` a cerca de 1,8:1. | Borda `--text-muted` (≥5,8:1) nos pontos vazados. |
| M8 | `lib/home/SettingsPanel.svelte:123-127`; `lib/session/SessionPill.svelte:335-338`; `routes/Gallery.svelte` (switch) | 1.4.11 | O estado pressionado se distingue por `--surface` contra `--surface-2` (1,10/1,22:1). Ajuda a mudança de cor do texto, mas o estado fica fraco. | Contorno ou sublinhado ≥3:1 no item `aria-pressed="true"`. |
| M9 | `lib/home/ContactCards.svelte:187-188` (36 px); `lib/session/SessionPill.svelte:308-309` (40 px), `:332-333` (32 px); `lib/PolicySelect.svelte:100`, `:145` (32 px); `lib/cards/Toggle.svelte:66-67` (40×24) | 2.5.5 (AAA; informativo, AA da 2.2 = 24 px passa) | Alvos abaixo de 44 px. | Aumentar a área de toque com padding ou um `::before` invisível, sem mudar o visual. |
| M10 | `lib/home/ContactCards.svelte:107`; `routes/Home.svelte:116` | 1.4.10 | Em 320 px, a Home rola 38 px na horizontal (`minmax(220px, 1fr)` + 32 px de padding lateral). | `minmax(min(220px, 100%), 1fr)` e padding `--space-4` abaixo de 400 px. |
| M11 | `lib/host/ConnectedBadge.svelte:29` | 2.4.6 | Com duas sessões há dois botões "Encerrar" iguais para o leitor de tela. | `label="Encerrar acesso de {badgeLabel(c)}"` (o rótulo visível continua "Encerrar"). |
| M12 | `routes/Connecting.svelte:171-174`; `App.svelte:66-75` | 2.4.3 / 4.1.3 | Ao trocar da Home para o Conectando, o foco cai em `BODY` (verificado). A live region nasce já com "Procurando a outra máquina", então o primeiro estado não é anunciado. | Focar o `<h1>` (`tabindex="-1"`) ao montar, ou deixar a live region montada no `App` antes de trocar de tela. |
| M13 | `lib/cards/Toggle.svelte:29-36` | 1.3.1 | O `hint` ("Só se a pessoa confirmou…") não está ligado ao switch. | `aria-describedby="h-{uid}"` no `<input>` e `id` na `.hint`. |
| M14 | `lib/session/SessionPill.svelte:185`, `:203` | 3.2.2 | Ativar uma ferramenta "em breve" (`aria-disabled`) tira o foco da pílula e manda para o canvas. | Não fazer nada no clique de um item `aria-disabled` (o anúncio "em breve" já existe). |
| M15 | `routes/Gallery.svelte` (dois `RequestCard` "Marta Souza") | best-practice | axe `landmark-unique`: duas `<section>` com o mesmo nome. É só a página de dev. | Fixtures com nomes diferentes. |

---

## O que passou

- **Contraste dos tokens (calculado, nos dois temas):** `--text`/`--bg` 17,7/17,5; `--text-muted` em bg/surface/surface-2/glass ≥5,79 (escuro) e ≥6,24 (claro); `--on-accent`/`--accent` 13,1/5,30; `--danger` no vidro 7,5/6,4; hover do danger (`--bg` sobre `--danger`) 7,9/6,0; `--warn` 11,7/6,2; o arco do anel (`--accent`) ≥4,8.
- **axe-core 4.13 (wcag2a/aa/21a/21aa + best-practice)** em `#home`, `#foco`, `#menu`, `#claro`, `#busca`, `#pedindo`, `#erro`, `#sessao`, `#conectado` e `#gallery`, nos temas escuro e claro: as únicas violações de contraste são as do I1. O resto é `region`/`page-has-heading-one`/`landmark-unique` (M1, M15). As sessões só reclamam de `region` no canvas e na pílula (aceitável no caso especial).
- **Foco visível:** todos os controles alcançados mostram `outline: 2px solid var(--ring)` (13,3:1 no escuro e 14,3:1 no claro sobre `--bg`). O anel não é o acento, como manda a regra.
- **Ordem de Tab na Home:** Configurações → campo → Conectar → para cada contato, cartão e "…". ↓ no campo leva ao primeiro cartão, e ↑/↓ navegam entre os cartões. Os cartões ficam `inert` enquanto recolhidos.
- **Conectando:** em erro, o foco vai para "Tentar de novo". Escape cancela nas fases canceláveis e não cancela durante a revelação. O campo de senha usa `Field` com rótulo visível e `autofocus`.
- **Sessão:** um toque em Alt chama e expande a pílula com foco. Tab percorre Monitor 1, Monitor 2, Qualidade, Conversa, Arquivos, Gravar, Voz e Encerrar. Escape devolve o teclado ao canvas. Os itens "em breve" usam `aria-disabled` + `aria-describedby`. As ferramentas recolhidas ficam `inert`, e a pílula escondida também.
- **Movimento reduzido:** com `reducedMotion: 'reduce'`, `document.getAnimations()` não tem nenhuma animação rodando com duração >0 em `#home`, `#foco`, `#busca`, `#sessao`, `#gallery` e `#conectado`. Os tokens `--dur-*` caem para 0. `motion.ts` zera a duração das Web Animations. O brilho do código, o traço do botão, a faísca do Conectando e o preenchimento do FileTrail têm alternativa estática.
- **Semântica:** `lang="pt-BR"`. Os ícones são decorativos (`aria-hidden`), e os botões só com ícone têm `label`. `Ring` usa `role="img"` com "N segundos para responder". `DropZone` e `FileTrail` usam `role="status"`. `ConnectedBadge` usa `aria-live="polite"`. Os erros do cartão (`AccessCard.svelte:109`) e da agenda (`Home.svelte:101`) usam `role="alert"`. `Toggle` é um checkbox nativo com `role="switch"` e `<label for>`. O select de política nomeia o que está em jogo.
- **Textos:** toda a interface auditada está em pt-BR.

## Método

1. Skill `design:accessibility-review` (checklist WCAG 2.1 AA: perceptível, operável, compreensível, robusto).
2. Leitura de todos os componentes do escopo, com números de linha.
3. Servidor Vite de dev (mock em `src/dev/mock.ts`) e Playwright 1.64 com o Chromium headless em cache. axe-core 4.13.0 injetado a partir de uma cópia local (offline), nos 10 hashes × 2 temas.
4. Teclado por script: sequência de Tab com `outline` calculado, Escape nos popovers, ↓ do campo, Alt + Tab na pílula, e overlay `session:msgbox` emitido pelo mock de eventos do Tauri.
5. Movimento reduzido emulado (`reducedMotion: 'reduce'`) e inspeção de `getAnimations()`.
6. Reflow em 640 px (equivalente a 200 % de zoom) e em 320 px, com capturas de tela.
7. Contraste: fórmula WCAG aplicada aos tokens, com composição alfa para `--surface-glass`, `--line*` e opacidades.
8. Capturas de `docs/design/momentos/*.png` revisadas.

Não foi feito: teste com leitor de tela real (VoiceOver/NVDA) e teste dentro do WebView do Tauri (só Chromium). Recomendo uma passada com VoiceOver no app depois de corrigir C1 e C2.
