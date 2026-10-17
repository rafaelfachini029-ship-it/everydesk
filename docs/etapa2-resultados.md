# Etapa 2 — Resultados (17/10/2026)

## Teste ponta a ponta (Mac `palco-som-74` ↔ notebook Windows `coroa-pulso-82`, mesma rede)
- Notebook com a build nova (CI run 38955801022) recebeu código memorável; entrou na fila de pendentes do servidor (com IP de origem) e foi aprovado; registrou em segundos.
- Mac → notebook: conexão direta (punch 81 ms). Cartão no notebook mostrou **identidade verificada**, nome, "Lembrar este contato" marcado por padrão. Aceite com período de 1 min → sessão abriu; Mac registrou "contacts updated from coroa-pulso-82".
- Notebook → Mac: cartão no Mac com oferta de pareamento (`key_changed=false`), aceito com "Sempre perguntar". Pareamento funciona nos dois sentidos.
- Reconfirmação: anel de 60 s apareceu no notebook após ~1 min; "Manter" não interrompeu a sessão (sem queda no log).
- Não testado (Rafael pediu para seguir): expiração dos 60 s, negar, revogar, mudança de política para "entrar direto", rede externa.
- Código digitado errado (`ora-pulso-82`) gerou "Failed to connect via rendezvous server" + dica de relógio — dica não se aplica a código inexistente; ajustar texto na Etapa 3.

## Estado do servidor
hbbs/hbbr com envelope assinado (±120 s, replay 300 s), relay com HMAC, OnlineRequest assinado (10/5 s por IP, 1/5 s por id), fila de pendentes, painel em https://everydesk.vpsfachini.online.

## Pendências levadas à Etapa 3
Ver `docs/superpowers/plans/2026-10-16-everydesk-etapa3-design.md` e os minors diferidos no ledger da Etapa 2 (rememberByDefault exigir verified; aceitar após negar sobrescreve política; texto da dica de relógio; UX de código regenerado/não autorizado).
