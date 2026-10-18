# EveryDesk: release

## Chave do updater

O app só instala atualizações assinadas com a chave minisign do EveryDesk. Até a chave
existir, `apps/everydesk/tauri.conf.json` traz um marcador e as atualizações ficam
**desligadas**: o app registra no log `updates disabled: updater pubkey is still the
placeholder`, não mostra nada e não instala nada.

1. No Mac, gere o par de chaves (uma vez só; a senha pedida protege a chave privada):

   ```sh
   cargo tauri signer generate -w "$HOME/everydesk-updater.key"
   ```

   Isso cria `~/everydesk-updater.key` (privada) e `~/everydesk-updater.key.pub` (pública).
   Guarde a privada e a senha no Bitwarden/1Password. Se perder a chave, nenhuma versão
   instalada consegue mais se atualizar sozinha.

2. Cole a chave **pública** em `apps/everydesk/tauri.conf.json`, em `plugins.updater.pubkey`,
   no lugar de `COLE_AQUI_A_CHAVE_PUBLICA_DO_UPDATER`. Use o conteúdo de
   `everydesk-updater.key.pub` do jeito que ele está, numa linha só (é base64):

   ```json
   "plugins": {
     "updater": {
       "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IC4uLg==",
   ```

   O teste `update::tests::configured_key_is_placeholder_or_valid`
   (`cargo test -p everydesk`) falha se o valor colado não for uma chave válida.

3. No GitHub, Environment `release`, crie os segredos `TAURI_SIGNING_PRIVATE_KEY` (conteúdo de
   `~/everydesk-updater.key`) e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. A chave privada nunca
   entra no repositório.

## O que o workflow de release precisa fazer (Task 8)

- `bundle.createUpdaterArtifacts` fica `false` no `tauri.conf.json`. Com `true`, o
  `cargo tauri build` falha quando `TAURI_SIGNING_PRIVATE_KEY` não está definido, o que
  quebraria o build de CI comum (sem segredos). O release liga a opção só ali:

  ```sh
  cargo tauri build --bundles nsis --features hwcodec \
    --config '{"bundle":{"createUpdaterArtifacts":true}}'
  ```

  Isso gera `EveryDesk_x.y.z_x64-setup.exe.sig` ao lado do setup.
- O portátil é assinado à parte, **com a versão** (o app recusa assinaturas sem versão,
  `requireSignedVersion: true`):

  ```sh
  cargo tauri signer sign --app-version x.y.z target/release/EveryDesk.exe
  ```

- `latest.json` (publicado como asset da release, o app lê
  `https://github.com/rafaelfachini029-ship-it/everydesk/releases/latest/download/latest.json`)
  tem dois alvos. `signature` é o conteúdo do arquivo `.sig`:

  ```json
  {
    "version": "x.y.z",
    "notes": "EveryDesk x.y.z",
    "pub_date": "2026-10-20T12:00:00Z",
    "platforms": {
      "windows-x86_64": {
        "url": "https://github.com/rafaelfachini029-ship-it/everydesk/releases/download/vx.y.z/EveryDesk_x.y.z_x64-setup.exe",
        "signature": "<conteúdo de EveryDesk_x.y.z_x64-setup.exe.sig>"
      },
      "windows-x86_64-portable": {
        "url": "https://github.com/rafaelfachini029-ship-it/everydesk/releases/download/vx.y.z/EveryDesk.exe",
        "signature": "<conteúdo de EveryDesk.exe.sig>"
      }
    }
  }
  ```

- Assine o portátil com Authenticode **antes** do `tauri signer sign`: a assinatura
  minisign cobre os bytes finais do arquivo.
- `/releases/latest/` ignora pré-releases. Enquanto só houver pré-releases, o app não encontra
  nada (ele fica quieto). Para testar o updater com uma rc, publique-a como release normal ou
  troque o endpoint numa build de teste.
