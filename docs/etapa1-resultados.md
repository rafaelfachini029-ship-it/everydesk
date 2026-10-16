# Etapa 1 — Resultados (16/10/2026)

## Teste ponta a ponta
- Controlador: Mac (ID 259402742, build dev). Acessado: notebook Windows Ghost Spectre (ID 247665637, `everydesk.exe` do CI run 38926933135, sem assinatura; SmartScreen contornado manualmente). Mesma rede local.
- Sessão 1: punch TCP ok, conexão direta não completou, **fallback para relay na VPS em 1,09 s** (caminho destravado pela correção C1). Cartão de aceite apareceu no notebook; vídeo AV1 apareceu no Mac. Mouse e teclado **não** foram aplicados no host, embora saíssem do Mac (diagnóstico confirmou `send_mouse`/`input_key` com permissão de teclado ativa).
- Sessão 2: conexão direta (8 ms). Vídeo, **mouse e teclado OK**.
- Pendentes (decisão de Rafael: adiar): Desconectar/indicador, Recusar, timeout de 60 s, dois monitores, rede externa (irmão), medição de latência/CPU/RSS em relay.

## Decisão para a Etapa 3 (vídeo nativo vs canvas)
Sem medição formal. Observação qualitativa: vídeo fluido em rede local via canvas. Manter canvas até medição; superfície nativa só se a latência medida exceder a meta (≤ 60 ms adicionais em 1080p30).

## Pendências técnicas levadas adiante
- Intermitência de entrada (sessão 1): investigar `active_mouse_`/`LATEST_PEER_INPUT_CURSOR` no host e diferença relay vs direto.
- APP_NAME ainda "RustDesk" (config/IPC compartilhados) → primeira tarefa da Etapa 2.
- Identidade autenticada do requisitante em punch/relay → Etapa 2 (pareamento por chave).
- Ruído de log: chamadas a API :21114 (sysinfo/heartbeat) que não existe → desligar na Etapa 2.
