# Crítica de design — Etapa 3 ("Painel de comando noturno")

Base: spec §6 (`docs/superpowers/specs/2026-10-16-acesso-remoto-design.md`), capturas em
`docs/design/momentos/` e `docs/design/gallery-*.png`, e leitura do código em
`apps/everydesk/ui/src`. Movimento avaliado pelo código (durações, easing, loops), não por
gravação. Prioridade: **Must** (quebra a direção ou a primeira impressão do dono), **Should**
(polimento visível), **Could** (refino).

---

## Visão geral

**Os cinco momentos leem como um produto só.** Fundo, superfícies, raio das pílulas, Geist +
Geist Mono e o verde-ciano aparecem com a mesma voz em todas as telas. A disciplina do acento está
quase toda certa: o código grande é neutro, o toggle do cartão é neutro, o "Identidade verificada"
usa a cor do texto e o foco usa `--ring`, não o acento. A tokenização (`design/tokens.css`,
`design/typography.css`, `design/motion.ts`) segura bem o sistema.

O que ainda impede o "uau" é menos uma questão de estilo e mais de **escala e silêncio**:

1. **A tela inicial não cresce com a janela.** Em 2560 px o código, o campo e o botão ocupam uma
   faixa estreita no meio de um vazio preto, e o rótulo do campo vira texto miúdo. A "placa" não
   parece placa.
2. **Há texto cinza pequeno demais**, o que o §6.1 proíbe: "Seu código", "disponível", "57 s",
   "código …", "Da próxima vez:", "está acessando", "automática 28 fps". O piso de 15 px está
   respeitado, mas o cinza no piso é exatamente o "texto pequeno cinza".
3. **A pílula expandida mostra quatro ferramentas "em breve"** (conversa, arquivos, gravar, voz).
   Isso é o oposto de "em dúvida, remover": metade da barra não faz nada.
4. **Loops infinitos** (faísca do "Conectando" e traço do botão em carregamento) se aproximam do
   spinner que o §6.2 proíbe.

O vermelho (`--danger`) é a única segunda cor. Ele está bem contido (encerrar, chave que não
confere, erro), mas aparece em quatro componentes. Isso é aceitável, desde que não se espalhe
mais.

---

## Momento 1 — Tela inicial

Capturas: `01-home-small.png`, `01-home-large.png`, `01-home-large-contatos.png`,
`01-home-huge.png`.

**O que funciona**
- O código em Geist Mono, com hífens rebaixados para cinza, lê como placa. É a melhor decisão
  visual do app.
- Há um único botão em acento ("Conectar"). O ponto "disponível" é a única outra marca de acento.
- O halo neutro que respira (`BigCode.svelte`, `.glow`) é sutil e sem cor, como manda a spec.
- Os contatos aparecem só ao focar o campo, como cartões discretos, e o ponto de online usa o
  acento corretamente.

**O que quebra a direção**
- **Escala:** `--fs-display` para em `7.5rem` (`typography.css`). Em `01-home-huge.png` o bloco
  ocupa cerca de 37% da largura e o campo cerca de 25%. O "tamanho gigante" some em telas grandes.
- **Hierarquia do rótulo:** "Para onde você quer ir?" está em 15 px, alinhado à esquerda, enquanto
  tudo acima está centralizado. É a frase-guia da tela e parece uma legenda de formulário.
- **Texto cinza pequeno:** "Seu código" e "disponível" ficam em `--text-muted` com tamanho de
  rótulo (`BigCode.svelte:49,100-112`).
- **Alinhamento:** o campo tem 64 px (`Field.svelte:74`) e o botão grande tem 60 px
  (`Button.svelte:95`). As bordas não se alinham, o que fica visível em `01-home-large.png`.
- **Engrenagem:** o ícone de configurações é pequeno e tem pouco contraste em telas grandes. Está
  correto ficar discreto, mas hoje parece perdido.
- **Texto do campo:** o placeholder diz "código ou contato" e a galeria diz "Código ou nome do
  contato". A caixa (minúscula ou maiúscula inicial) também varia entre telas.

**Sugestões**
- **Must:** escalar o bloco com a janela. Subir o teto de `--fs-display` (por exemplo para
  `clamp(3.5rem, 11vw, 12rem)`) e fazer a largura de `GoField` acompanhar a largura do código
  (`min(100%, 40rem)` hoje, proporcional ao código depois). Arquivos: `design/typography.css`,
  `lib/home/GoField.svelte`, `routes/Home.svelte`.
- **Must:** promover "Para onde você quer ir?" a `t-lead`, centralizado, em `--text`. Ele é a
  pergunta da tela. Arquivo: `lib/home/GoField.svelte`.
- **Should:** "Seu código" e "disponível" em `--fs-body` ou maiores, e "disponível" em `--text`
  (o ponto já carrega o estado). Arquivo: `lib/home/BigCode.svelte`.
- **Should:** igualar a altura de campo e botão em 64 px. Arquivos: `lib/ui/Button.svelte` (`.lg`)
  e `lib/ui/Field.svelte`.
- **Could:** unificar o placeholder ("código ou contato") e a caixa dos rótulos em todo o app.

---

## Momento 2 — Conectando

Capturas: `02-conectando-busca.png`, `02-conectando-erro.png`.

**O que funciona**
- O traço de luz entre "Você" e o destino é exatamente a metáfora da spec. O cartão de destino
  esmaecido até ser encontrado comunica o estado sem palavras.
- No erro o traço apaga para cinza, sem vermelho, o que é calmo e correto. "Tentar de novo" em
  acento continua sendo "botão de conectar".
- A mensagem de status é grande (`--fs-lead`) e em `--text`, sem cinza pequeno.

**O que quebra a direção**
- **Continuidade:** a tela inicial tem o código gigante centralizado. "Conectando" troca para dois
  cartões pequenos nas laterais, e não há transição de elemento compartilhado. O "seu cartão" de
  onde o traço sai não é a placa que o usuário acabou de ver.
- **Loop infinito:** a faísca (`connecting.ts:162`, `iterations: Infinity`, 1400 ms) e o `.trace`
  do botão (`Button.svelte:162`, 1,2 s infinito) funcionam como spinners. A spec diz "sem spinner"
  e pede movimento de 150 a 400 ms com propósito.
- **Composição:** os cartões ficam a mais de 600 px um do outro em 2560 px. O centro fica vazio e o
  traço parece fino e pequeno demais para ser o herói da tela.
- **Texto cinza:** os códigos nos cartões estão em `--fs-label` e `--text-muted`
  (`Connecting.svelte:240-241,377-378`). No destino esmaecido ficam quase ilegíveis.
- **Voz:** "Não encontrei essa máquina" (`connecting.ts:19`) usa a primeira pessoa do singular. A
  galeria e a tela inicial usam "Não encontramos esse código". O produto fala com duas vozes.
- **Foco:** o anel de foco duplo e branco em volta do botão de acento, no erro, pesa mais que o
  próprio botão.

**Sugestões**
- **Must:** trocar a faísca infinita por um único percurso. O traço avança até onde a busca chegou
  (`reach`) e fica parado, com no máximo um brilho lento de opacidade no ponto final. O loop do
  botão também sai (`lib/ui/Button.svelte`, `.trace`). Arquivos: `routes/Connecting.svelte`,
  `lib/connecting/connecting.ts`.
- **Should:** fazer o cartão "Você" nascer da placa da tela inicial: o código encolhe até a posição
  do cartão em cerca de 400 ms (View Transitions ou FLIP via `design/motion.ts`). Isso amarra o
  momento 1 ao 2.
- **Should:** aproximar os cartões (largura máxima de aproximadamente 960 px) e engrossar o traço
  para 2 a 3 px com mais brilho, para que a luz seja o herói.
- **Should:** unificar a voz das mensagens. Sugestão: "Não encontramos essa máquina. Confira o
  código." Arquivo: `lib/connecting/connecting.ts`.
- **Could:** códigos dos cartões em `--fs-body` e `--text`. O esmaecimento do destino vem só da
  opacidade do cartão.

---

## Momento 3 — Sessão e pílula

Capturas: `03-sessao-pilula-colapsada.png`, `03-sessao-pilula-expandida.png`.

**O que funciona**
- A tela remota ocupa 100% da janela, sem barra fixa. A pílula colapsada ("33 ms · Mãe · ○") é
  exatamente o objeto da spec: pequena, escura e flutuante, com sombra que a separa de qualquer
  fundo.
- O "Encerrar" isolado à direita, em vermelho contornado, é inequívoco.
- Ícones de traço fino e consistentes (Lucide).

**O que quebra a direção**
- **Ferramentas mortas:** quatro dos controles expandidos são "em breve"
  (`SessionPill.svelte:48-54`) e o tooltip "Gravar · em breve" aparece no hover. Para o dono, isso
  lê como inacabado, que é o pior sinal possível no momento de "uau".
- **Densidade:** a pílula expandida ocupa cerca de 67% da largura da tela, com monitores
  ("1 2"), qualidade ("automática"), fps, quatro ícones, dois divisores e Encerrar. Isso já é uma
  barra, e a spec diz "nenhuma barra".
- **Ordem:** a spec lista arquivos, gravar, monitores, chat, qualidade, encerrar. A pílula da
  galeria segue outra ordem e mostra Encerrar só como ícone. A `SessionPill` real segue uma
  terceira ordem, com Encerrar rotulado. São dois componentes que deveriam ser um.
- **Texto cinza pequeno:** "automática" e "28 fps" em `--fs-label` e `--text-muted`
  (`SessionPill.svelte:283-284,348-349`).
- **Ponto de voz:** um círculo vazio cinza com 10 px não diz "voz" a ninguém. Na colapsada parece
  um defeito de renderização.
- **Latência:** "33 ms" em mono com o espaço largo da fonte mono fica desequilibrado ao lado de
  "Mãe" em Geist.

**Sugestões**
- **Must:** esconder as ferramentas "em breve" da pílula nesta etapa. Mostrar só o que funciona
  (monitores, qualidade, encerrar). O espaço vazio é a direção. Arquivo:
  `lib/session/SessionPill.svelte` (array `soon`).
- **Should:** fundir a pílula da galeria (`lib/ui/Pill.svelte` + `routes/Gallery.svelte`) e a
  `SessionPill` numa ordem só, alinhada à spec, para que a galeria seja a fonte de verdade.
- **Should:** fps só aparece ao passar o mouse sobre "qualidade". "automática" em `--text`.
- **Should:** tirar o ponto de voz enquanto a voz não existir, ou trocá-lo por um ícone de
  microfone de traço fino.
- **Could:** "33 ms" com o número em mono e a unidade em Geist, com espaço fino entre os dois.

---

## Momento 4 — Cartões de confirmação e reconfirmação

Capturas: `04-cartoes-dark.png`, `04-cartoes-light.png`.

**O que funciona**
- É o momento mais maduro. O nome em 28 px domina, o anel de 60 s em acento ao redor das iniciais
  é exatamente o "anel de tempo" da spec, e o mesmo componente (`RequestCard`) serve ao pedido e à
  reconfirmação, só com texto diferente, como pede a spec.
- O alerta de chave que não confere é inequívoco sem ser histérico.
- O tema claro preserva a hierarquia. O acento escurecido (`#007a62`) mantém contraste.
- O toggle neutro protege a exclusividade do acento.

**O que quebra a direção**
- **Redundância:** o anel já mostra o tempo, e "57 s" no canto, em cinza, repete a informação como
  texto pequeno (`RequestCard.svelte:56,135-138`).
- **Acento em "Aceitar" e "Manter":** é defensável, pois é a ação de conexão do lado acessado, mas
  no cartão com chave que não confere o botão em acento convida a aceitar justamente o caso
  perigoso.
- **Texto cinza:** "código lua-azul-sete" e "Da próxima vez:" estão em `--text-muted`. O primeiro
  é informação de segurança e não deveria parecer secundário.
- **Densidade do primeiro cartão:** nome, verbo, código, identidade, divisor, toggle, select e dois
  botões. Para um cartão que "desliza no canto", é muito.

**Sugestões**
- **Must:** no cartão com chave que não confere, "Aceitar" vira botão secundário (contornado) e
  "Recusar" vira o padrão. A cor não pode empurrar para o risco. Arquivos:
  `lib/cards/AccessCard.svelte`, `lib/cards/cards.ts`.
- **Should:** remover o "57 s" textual, ou colocar o número dentro do anel no lugar das iniciais
  nos últimos 10 s. O `aria-label` do anel já cobre o leitor de tela.
- **Should:** código em `--text` e `--fs-body`, e "Da próxima vez:" em `--text`.
- **Could:** recolher "Lembrar este contato" e a política atrás de uma linha só ("Lembrar · sempre
  perguntar ▾") para encurtar o cartão.

---

## Momento 5 — Arquivos e o selo "alguém conectado"

Capturas: `05-arquivos-arrastando.png`, `05-badge-conectado.png`.

**O que funciona**
- A zona de soltar está certa: a tela escurece, quatro cantos finos marcam a área, e uma pílula
  central diz "Solte para enviar para Mãe". É silenciosa e precisa. É o melhor exemplo da
  direção no app.
- O selo usa o acento só no ponto de status, estável e sem pulsar. A escolha está documentada
  (`ConnectedBadge.svelte:69`).

**O que quebra a direção**
- **Selos empilhados:** duas pílulas de larguras diferentes, coladas no canto inferior esquerdo,
  ficam visualmente desalinhadas. O identificador cru "pedra-gaviao-90" convive com um nome
  ("Mãe").
- **Texto cinza:** "está acessando" em `--text-muted` e tamanho de rótulo
  (`ConnectedBadge.svelte:59,92`).
- **Peso do vermelho:** "Encerrar" em vermelho em cada selo, mais o vermelho de outros momentos,
  começa a fazer do vermelho uma segunda cor de destaque.
- **Contraste dos cantos:** os cantos brancos da zona de soltar têm pouco contraste no claro do
  remoto (o fundo de exemplo vai de verde para lilás) e somem nas áreas claras.
- **Trilha "em breve":** a `FileTrail` diz "Transferência de arquivos chega em breve". Mesmo
  problema do momento 3: prometer em vez de entregar.

**Sugestões**
- **Should:** quando houver mais de um acesso, juntar os selos numa pílula só ("2 pessoas
  acessando ▾") com largura fixa. Com um acesso, manter o selo atual. Arquivo:
  `lib/host/ConnectedBadge.svelte`.
- **Should:** "está acessando" em `--text`. "Encerrar" em `--text` com sublinhado no hover,
  deixando o vermelho só para a confirmação ou para o ícone. Arquivo:
  `lib/host/ConnectedBadge.svelte`.
- **Could:** dar aos cantos da zona de soltar uma sombra escura de 1 px, para que apareçam sobre
  qualquer fundo. Arquivo: `lib/session/DropZone.svelte`.
- **Could:** na demonstração ao dono, soltar um arquivo e mostrar a trilha enchendo, não a frase
  "em breve".

---

## Lista de polimento antes de mostrar ao dono (máx. 10)

1. **[Must]** Tela inicial escala com a janela: teto maior em `--fs-display`, e o campo acompanha a
   largura do código (`typography.css`, `GoField.svelte`, `Home.svelte`).
2. **[Must]** "Para onde você quer ir?" vira `t-lead`, centralizado, em `--text` (`GoField.svelte`).
3. **[Must]** Tirar as ferramentas "em breve" da pílula da sessão (`SessionPill.svelte`).
4. **[Must]** Faísca e traço do botão sem loop infinito: o traço avança e para
   (`connecting.ts`, `Connecting.svelte`, `Button.svelte`).
5. **[Must]** Cartão com chave que não confere: "Aceitar" secundário, sem acento
   (`AccessCard.svelte`, `cards.ts`).
6. **[Should]** Varredura de "cinza pequeno": `--text-muted` só em `--fs-body` ou maior, nunca em
   informação de segurança. Afeta BigCode, Connecting, RequestCard, AccessCard, SessionPill e
   ConnectedBadge.
7. **[Should]** Remover o "57 s" textual do cartão. O anel basta (`RequestCard.svelte`).
8. **[Should]** Unificar a voz das mensagens ("encontramos") e o placeholder entre galeria e telas
   (`connecting.ts`, `GoField.svelte`, `Gallery.svelte`).
9. **[Should]** Pílula única: mesma ordem e mesmo Encerrar na galeria e na sessão. Ponto de voz
   fora enquanto a voz não existir (`Pill.svelte`, `SessionPill.svelte`, `Gallery.svelte`).
10. **[Should]** Campo e botão com a mesma altura de 64 px; selos múltiplos agrupados numa pílula
    (`Button.svelte`, `Field.svelte`, `ConnectedBadge.svelte`).
