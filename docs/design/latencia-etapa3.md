# Latência e fps na sessão: Etapa 3 (pílula flutuante)

Regra global: a camada de UI nunca reduz o fps do canvas. Este arquivo diz o que já foi feito para isso e o que medir no próximo E2E.

## O que não deu para medir aqui

`session:quality` (fps, delay) só existe com um par real do outro lado. Nesta máquina não há segundo computador, então o "antes/depois com a pílula visível" fica para o E2E.

## Sonda de tempo de quadro (só em dev)

`apps/everydesk/ui/src/lib/RemoteCanvas.svelte`, em `onFrame`: em `import.meta.env.DEV`, o `putImageData` é cronometrado com `performance.now()`.
A cada 120 quadros sai no console (formato; os números aqui são ilustrativos, não medidos):

    [frame] putImageData avg 0.84 ms, max 2.10 ms

Em build de produção a sonda não existe (o ramo é eliminado pelo Vite) e o `putImageData` roda direto.

## Custos evitados de propósito

- **Sem `backdrop-filter` sobre o canvas.** O `Pill` base usa blur (`blur(18px)`). Um blur sobre um canvas que repinta a cada quadro obriga o compositor a refazer o desfoque a cada quadro. A `SessionPill` troca isso por uma superfície opaca (`--surface`), que custa só uma composição simples.
- **Aparecer e sumir só com `transform` e `opacity`.** Nada de top/height, então não há relayout da tela remota. A largura da pílula anima dentro dela (grid 0fr→1fr) e não mexe no canvas, que é `position: fixed`, irmão dela.
- **Nada de estado reativo por quadro.** `currentDisplay` só recebe `set` quando o monitor muda. `displays`/`current` no RemoteCanvas são variáveis simples, não `$state`. Os `$state` usados por quadro (`revealed`, `connected`) só mudam uma vez.
- **Zona morta barata.** É um `getBoundingClientRect()` da pílula por evento de ponteiro, e só enquanto ela está visível. Com a pílula escondida, `hitRect()` devolve `null` sem tocar no layout.
- **Latência sem animação.** O número só troca de texto (1×/s, no ritmo do `session:quality`).

## O que medir no próximo E2E (host real, 1080p e 4K)

1. **fps de `session:quality` em três estados**, 60 s cada, com a tela remota em movimento (vídeo):
   - (a) pílula escondida
   - (b) pílula colapsada visível
   - (c) pílula expandida com o ponteiro parado sobre ela

   Critério: a mediana de (b) e (c) fica dentro de 1 fps de (a).
2. **Sonda `[frame] putImageData`**: avg/max nos mesmos três estados. Critério: o max não sobe mais que 1 ms com a pílula visível.
3. **Chrome/WebKit Performance**, gravando 10 s em (c): procurar "Recalculate Style"/"Layout" por quadro. O esperado é nenhum, além do hover.
4. **`delay` mostrado na pílula contra o atraso percebido** (cronômetro filmado nas duas telas), para validar que o "32 ms" é o número certo de exibir.
5. **Comparação com o `Pill` original (com blur)**, para registrar quanto o blur custaria se alguém quiser voltar com ele.
