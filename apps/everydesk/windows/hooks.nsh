; Hooks do instalador NSIS do EveryDesk (Tauri 2: bundle.windows.nsis.installerHooks).
; O Tauri inclui este arquivo no topo do installer.nsi; as macros abaixo são expandidas
; dentro das seções Install/Uninstall, depois de SetContext (SetShellVarContext all e
; SetRegView 64), então HKLM aqui é a visão nativa de 64 bits.
; Aviso: o próprio Tauri já mostra o checkbox "Executar EveryDesk" na página final e o
; dispara com nsis_tauri_utils::RunAsUser (sem elevação). Por isso nenhum hook abre o app.

; Nome do serviço do Windows criado pelo motor (`sc create EveryDesk ...`).
!define EVERYDESK_SERVICE "EveryDesk"
; Chave de inicialização automática para todos os usuários (HKLM).
!define EVERYDESK_RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
; Nome do valor dentro da chave Run.
!define EVERYDESK_RUN_VALUE "EveryDesk"

; Antes de copiar os arquivos: tira o serviço do caminho para o exe não ficar travado.
; Também roda na atualização automática (o app executa o setup com /P /UPDATE /R): em /UPDATE o
; Tauri não chama o desinstalador antigo, então este PREINSTALL e o POSTINSTALL abaixo bastam.
!macro NSIS_HOOK_PREINSTALL
  ; Consulta se o serviço EveryDesk já existe (código 0 = existe; 1060 = não existe).
  nsExec::ExecToStack 'sc query "${EVERYDESK_SERVICE}"'
  ; Primeiro valor na pilha: código de saída do sc.
  Pop $0
  ; Segundo valor na pilha: texto da saída (descartado).
  Pop $1
  ; Só mexe no serviço se ele existir (instalação anterior).
  ${If} $0 = 0
    ; Se o exe antigo ainda está lá, deixa o próprio motor remover o serviço.
    ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
      ; `--uninstall-service`: sc stop + sc delete + apaga o valor Run + mata os outros EveryDesk.exe.
      ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-service'
    ${EndIf}
    ; Garantia caso o exe antigo falhe ou não exista: para o serviço direto.
    nsExec::Exec 'sc stop "${EVERYDESK_SERVICE}"'
    ; Descarta o código de saída do sc stop (pode já estar parado).
    Pop $0
    ; Garantia: apaga o serviço direto (ignora erro se já foi apagado).
    nsExec::Exec 'sc delete "${EVERYDESK_SERVICE}"'
    ; Descarta o código de saída do sc delete.
    Pop $0
  ${EndIf}
  ; Mata qualquer EveryDesk.exe restante (app, --server, --cm); erro ignorado se não houver.
  nsExec::Exec 'taskkill /F /IM "${MAINBINARYNAME}.exe"'
  ; Descarta o código de saída do taskkill (128 = nenhum processo encontrado).
  Pop $0
!macroend

; Depois de copiar os arquivos e criar os atalhos: instala o serviço e o autostart.
!macro NSIS_HOOK_POSTINSTALL
  ; Mostra no log de detalhes do instalador o que está acontecendo.
  DetailPrint "Instalando o serviço de acesso permanente do EveryDesk..."
  ; `--install-service`: importa a config do usuário para o perfil SYSTEM, faz sc create/start
  ; e já grava o valor Run `--hidden` (o botão "Instalar" do app usa o mesmo comando).
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --install-service' $0
  ; Registra o código de saída no log de detalhes (0 = ok; o motor também grava log próprio).
  DetailPrint "Serviço do EveryDesk: código de saída $0"
  ; Inicia o app escondido (só bandeja/CM) no login de qualquer usuário. Regrava o mesmo valor
  ; que o `--install-service` (idempotente), para o autostart existir mesmo se ele falhou.
  WriteRegStr HKLM "${EVERYDESK_RUN_KEY}" "${EVERYDESK_RUN_VALUE}" '"$INSTDIR\${MAINBINARYNAME}.exe" --hidden'
  ; Abrir o app agora fica com a página final do Tauri (RunAsUser, sem elevação).
!macroend

; Antes de remover arquivos/registro/atalhos: tira serviço, autostart e processos.
!macro NSIS_HOOK_PREUNINSTALL
  ; Se o exe instalado existe, deixa o motor remover o serviço (sc stop + sc delete) e o
  ; valor Run (igual ao botão "Remover acesso permanente" do app).
  ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    ; `--uninstall-service` roda sem janela e encerra os outros EveryDesk.exe.
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-service'
  ${EndIf}
  ; Garantia: para o serviço direto (ignora erro se ele não existir mais).
  nsExec::Exec 'sc stop "${EVERYDESK_SERVICE}"'
  ; Descarta o código de saída do sc stop.
  Pop $0
  ; Garantia: apaga o serviço direto (ignora erro se ele não existir mais).
  nsExec::Exec 'sc delete "${EVERYDESK_SERVICE}"'
  ; Descarta o código de saída do sc delete.
  Pop $0
  ; Numa atualização (/UPDATE) não apaga aqui; o POSTINSTALL da nova versão regrava o valor
  ; de qualquer forma (o `--uninstall-service` acima já pode tê-lo apagado).
  ${If} $UpdateMode <> 1
    ; Desinstalação de verdade: remove a inicialização automática.
    DeleteRegValue HKLM "${EVERYDESK_RUN_KEY}" "${EVERYDESK_RUN_VALUE}"
  ${EndIf}
  ; Mata qualquer EveryDesk.exe restante para os arquivos poderem ser apagados.
  nsExec::Exec 'taskkill /F /IM "${MAINBINARYNAME}.exe"'
  ; Descarta o código de saída do taskkill.
  Pop $0
!macroend

; Depois de remover arquivos/registro/atalhos.
!macro NSIS_HOOK_POSTUNINSTALL
  ; Nada a fazer: %AppData%\EveryDesk (config, pareamentos, contatos) é preservado de propósito.
  ; O checkbox "apagar dados" do Tauri só apaga %AppData%\br.com.everydesk.app (identifier),
  ; que não é a pasta do EveryDesk; a cópia no perfil SYSTEM (ServiceProfiles) também fica.
!macroend
