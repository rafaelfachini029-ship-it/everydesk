# EveryDesk — caminhos em disco

O plano da Etapa 2 citava `~/Library/Application Support/EveryDesk` no Mac. Esse caminho está errado:
o `hbb_common` monta o diretório de config a partir de `ORG` + `APP_NAME`
(`online.vpsfachini` + `EveryDesk`).

| O quê | macOS | Windows |
|---|---|---|
| Config (`EveryDesk.toml`, `EveryDesk2.toml`) | `~/Library/Preferences/online.vpsfachini.EveryDesk/` | `%AppData%\EveryDesk\config\` |
| Livro de pareamentos do host (`pairings.json`, cifrado) | mesmo diretório da config | mesmo diretório da config |
| Contatos do controlador (`contacts.json`, cifrado) | mesmo diretório da config | mesmo diretório da config |
| Livro ilegível (a chave da máquina mudou) | `pairings.json.bak-<ts>` / `contacts.json.bak-<ts>` ao lado | idem |
| IPC | `/tmp/EveryDesk-<uid>/ipc_cm` | `\\.\pipe\EveryDesk\query_cm` |

A config **não** é compartilhada com uma instalação do RustDesk (o `APP_NAME` é `EveryDesk`
desde a Tarefa 1 da Etapa 2).
