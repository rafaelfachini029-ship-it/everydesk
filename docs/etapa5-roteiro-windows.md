# Etapa 5 — Roteiro de teste no Windows (notebook)

Faça na ordem. Anote o que der diferente do esperado.

1. **Instalar pelo setup.** Rode o instalador (aceite o pedido de administrador uma vez). Esperado: o app reabre sem ser administrador e mostra o mesmo código do portátil. Em um prompt de administrador, `sc query EveryDesk` mostra RUNNING.
2. **Instalar com usuário comum.** Repita com uma conta sem administrador, digitando a senha de um administrador. Anote qual código aparece depois.
3. **Configurações.** Ligue e desligue permissões no app. Abra os links da tela "Sobre".
4. **Tela bloqueada.** Bloqueie o notebook e conecte do Mac: com contato "perguntar sempre" e com contato "entrar direto".
5. **Antes do login.** Reinicie o notebook e, sem fazer login, conecte do Mac. "Entrar direto" entra. "Perguntar sempre" e transferência de arquivos são recusados com a mensagem "Ninguém está usando…". Depois faça login: a sessão continua e o app aparece em até 1 minuto.
6. **Revogar.** Durante uma sessão antes do login, revogue o contato no app: a sessão cai.
7. **Segundo usuário.** Faça login com outra conta do Windows e veja a lista de contatos.
8. **Instalar pelo portátil.** No portátil, use "Instalar para acesso permanente". Depois "Remover acesso permanente": o app volta a ser portátil e não abre sozinho no próximo login. Depois "Reiniciar serviço".
9. **Setup com o portátil aberto.** Rode o instalador com o portátil aberto: o app deve fechar e reabrir sozinho.
10. **Atualização (instalado).** Quando houver a versão `v0.1.0-rc2`: o app oferece atualizar; com sessão aberta pede confirmação; recusar o pedido de administrador deixa o app visível; depois de atualizar, o código e os contatos continuam iguais.
11. **Atualização (portátil).** Portátil em Downloads atualiza sozinho. Portátil numa pasta sem permissão de escrita mostra mensagem de erro e não derruba a sessão.
12. **Desinstalar.** Desinstale pelo Windows. `sc query EveryDesk` responde erro 1060 e nenhum processo EveryDesk fica aberto. Suas configurações em `%AppData%\EveryDesk` continuam lá.
13. **Assinatura** (quando houver certificado). `signtool verify /pa /v` no instalador e no portátil passa.
