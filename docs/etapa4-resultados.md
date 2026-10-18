# Etapa 4 — Resultados (18/10/2026)

## Entregue
- Arquivos: arrastar do controlador para a tela remota envia para a Área de Trabalho (ou Downloads) do host, com trilha de progresso na pílula, "Preparando…", pergunta de sobrescrita, cancelamento; nomes seguros; pastas verificadas antes do envio; caminhos só do evento nativo de drop/seletor Rust.
- Segurança (revisão final): ações de arquivo em sessão de tela limitadas a upload confinado (Desktop/Downloads), sem listar/baixar/apagar; detecção de sobrescrita forçada; host vê log de transferências no badge e tem interruptores de permissão (arquivos, som, gravação, área de transferência) nas configurações.
- Conversa por texto nos dois lados; voz com um clique (consentimento no host, timeout de 60 s); gravação com indicador nos dois lados (WebM por padrão; MP4 só com hwcodec); extras: só olhar (fail-closed, bloqueia também arquivos), bloquear entrada, som, área de transferência, cursor remoto, qualidade, reiniciar, Ctrl+Alt+Del, bloquear tela; elevação automática no Windows com cartão único em português e memória da escolha.
- macOS: Info.plist (microfone) e Entitlements (audio-input) para builds assinadas.

## Não testado (Rafael pediu para seguir)
Teste real de todas as funções entre Mac e notebook (CI run 39054119784). Itens só verificáveis ali: destino do arquivo com OneDrive, áudio nos dois sentidos, arquivo de gravação após queda, UAC/elevação.

## Pendências levadas à Etapa 5
`--server` separado (modo serviço) precisa receber permissões via IPC; residuais listados no ledger da Etapa 4.
