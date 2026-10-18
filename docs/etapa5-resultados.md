# Etapa 5 — Distribuição: resultados

**Status:** código concluído e revisado; build do Windows verde (run 39163199141, commit 77cfa075f) com instalador (13,2 MB) e portátil (18,5 MB). Teste no Windows pendente (roteiro em `docs/etapa5-roteiro-windows.md`). Release assinada pendente do certificado.

## Entregue
- Instalador NSIS (por máquina) e portátil a partir da mesma build.
- Ganchos do instalador: serviço do Windows, início automático escondido, desinstalação limpa; importação dos pareamentos só na primeira instalação.
- Modo serviço no app: detecção, "Instalar para acesso permanente" (pede administrador uma vez), "Remover", "Reiniciar serviço", reabertura sem administrador.
- Acesso antes do login só para contatos "entrar direto" autenticados.
- Configurações e permissões chegam ao serviço; erro claro se ele não responder.
- Livro de contatos servido pelo serviço (listar, mudar política, revogar; nunca adicionar); revogar encerra a sessão no próprio serviço.
- Atualização automática (instalado e portátil) com assinatura obrigatória e recusa de versão antiga.
- Workflow de release assinado (compilar sem segredos, assinar em job separado com aprovação, publicar).
- Endurecimento: instalar pelo portátil copia só o exe; DLLs só do System32.

## Pendências do Rafael
1. Gerar a chave do updater e colar a pública em `tauri.conf.json` (`plugins.updater.pubkey`).
2. Comprar SSL.com IV + eSigner; criar Environment `release` e os seis segredos; preencher `CODESIGNTOOL_SHA256`.
3. Confirmar o nome do publicador igual ao do certificado.
4. Tornar o repositório público (AGPL).
5. Rodar o roteiro no notebook.

## Adiados (menores)
Script órfão em %TEMP% se o UAC for recusado; perda rara de atualização em gravações simultâneas de opções; senha do eSigner visível na lista de processos do runner temporário; espera de até ~260 s e fila sem limite no acesso antes do login; `nsExec.dll` sem assinatura.
