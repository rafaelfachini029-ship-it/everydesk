# EveryDesk: release

Este guia é para publicar uma versão assinada do EveryDesk para Windows. Os comandos vêm um
por bloco: copie e cole um de cada vez.

## Como funciona

Quando uma tag que começa com `v` (por exemplo `v0.1.0`) é enviada ao GitHub, o workflow
`.github/workflows/everydesk-release.yml` começa a rodar:

1. **Compila, sem segredos** (job `build`, de 1 a 2 horas). Antes de compilar, ele confere a
   tag, a versão em `apps/everydesk/tauri.conf.json` e a chave do updater. Se algo estiver
   errado, ele para em poucos segundos e mostra uma mensagem em português. Neste job nenhum
   segredo existe: nada do que é compilado consegue ler o certificado ou a chave.
2. **Espera a sua aprovação** (job `sign`). Ele usa o Environment `release`, que tem você como
   revisor obrigatório. Quando a compilação termina, o GitHub pede a aprovação. Nada é
   assinado (e nenhum segredo é lido) até você clicar em **Approve and deploy** na página do
   workflow, na aba **Actions**. O job roda numa máquina nova e limpa. Primeiro ele confere se
   todos os segredos existem.
3. **Monta e assina o instalador**, sem compilar nada de novo. Usa o certificado da SSL.com
   (eSigner) no `EveryDesk.exe` de dentro do instalador, no desinstalador, nos plugins do
   instalador e no próprio `EveryDesk_x.y.z_x64-setup.exe`. Toda assinatura leva carimbo de
   tempo (RFC 3161), então continua válida depois que o certificado vencer.
4. **Assina o portátil** `EveryDesk.exe` com o mesmo certificado.
5. **Confere as assinaturas** com `signtool verify /pa /v`, nos dois arquivos e em cada
   executável de dentro do instalador.
6. **Assina para o updater.** O instalador e o portátil recebem cada um um `.sig` (chave
   minisign do updater), com a versão gravada na assinatura. O portátil só é assinado para o
   updater **depois** do Authenticode, para o `.sig` cobrir o arquivo final.
7. **Gera `latest.json`.** É o arquivo que os EveryDesk instalados consultam. Antes de gravá-lo,
   o workflow confere os dois `.sig` com a chave pública do `tauri.conf.json`.
8. **Publica a release** (job `publish`, o único com permissão de escrita no repositório) no GitHub com `EveryDesk_x.y.z_x64-setup.exe`, `EveryDesk.exe`, os dois
   `.sig`, `latest.json`, `LICENCE` e `SHA256SUMS`.

Tags com sufixo (`v0.1.0-rc1`, `v0.2.0-beta.1`) viram **pré-release**. Tags sem sufixo
(`v0.1.0`) viram release normal e passam a ser a "última versão".

O workflow comum (`everydesk-windows.yml`, a cada push) continua gerando arquivos **sem
assinatura** e sem usar segredo nenhum. A assinatura só existe no release.

Nada é compactado nem ofuscado (sem UPX ou packers): isso evita alarme falso de antivírus.

## 1. Preparar uma vez

### 1.1 Chave do updater

O app só instala atualizações assinadas com a chave minisign do EveryDesk. Até a chave
existir, `apps/everydesk/tauri.conf.json` traz um marcador e as atualizações ficam
**desligadas**. O app registra no log `updates disabled: updater pubkey is still the
placeholder`, não mostra nada e não instala nada.

O release **normal** (`v0.1.0`) se recusa a sair com o marcador. A pré-release (`-rc`) sai,
com um aviso.

1. No Mac, gere o par de chaves uma vez só. A senha pedida protege a chave privada:

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

   O teste `update::tests::configured_key_is_placeholder_or_valid` falha se o valor colado não
   for uma chave válida. Rode-o assim:

   ```sh
   cargo test -p everydesk
   ```

3. A chave privada vai para o GitHub como segredo (passo 1.3). Ela nunca entra no repositório.

### 1.2 Certificado de assinatura (SSL.com eSigner)

1. Compre na SSL.com um certificado **Code Signing IV** (Individual Validation, no seu nome)
   com **eSigner** (assinatura na nuvem, sem token USB).
2. Depois da validação, ative o eSigner no painel da SSL.com. Na hora de escolher o segundo
   fator, escolha **OTP App** / "2FA app". A tela mostra um QR code e um **código secreto**
   (texto longo). Guarde esse código: ele vira o segredo `ES_TOTP_SECRET`. Ele só aparece
   nessa tela.
3. No painel do pedido, copie o **Credential ID** do certificado (é o `ES_CREDENTIAL_ID`).
4. Guarde também o usuário e a senha da conta SSL.com (`ES_USERNAME`, `ES_PASSWORD`).

**Cota.** Cada release faz 9 assinaturas no eSigner: o `EveryDesk.exe` interno, 5 plugins do
instalador, o desinstalador, o setup e o portátil. Confira quantas assinaturas por mês o seu
plano permite.

### 1.3 Environment `release` no GitHub

No navegador, abra o repositório
`https://github.com/rafaelfachini029-ship-it/everydesk` e siga estes passos.

1. **Settings** → **Environments** → **New environment**. Nome: `release` → **Configure
   environment**.
2. Marque **Required reviewers** e adicione você mesmo (`rafaelfachini029-ship-it`).
   Clique em **Save protection rules**.
3. Em **Deployment branches and tags**, escolha **Selected branches and tags** → **Add
   deployment branch or tag rule** → tipo **Tag**, padrão `v*`.
4. Em **Environment secrets** → **Add environment secret**, crie um por um:

   | Nome | Valor |
   |---|---|
   | `ES_USERNAME` | usuário da conta SSL.com |
   | `ES_PASSWORD` | senha da conta SSL.com |
   | `ES_CREDENTIAL_ID` | Credential ID do certificado |
   | `ES_TOTP_SECRET` | código secreto do OTP App (passo 1.2) |
   | `TAURI_SIGNING_PRIVATE_KEY` | conteúdo inteiro de `~/everydesk-updater.key` |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | senha da chave do updater |

   Para copiar a chave privada no Mac (vai para a área de transferência, sem aparecer na tela):

   ```sh
   pbcopy < "$HOME/everydesk-updater.key"
   ```

5. Em **Environment variables** → **Add environment variable**:

   | Nome | Valor |
   |---|---|
   | `ESIGNER_ENV` | `TEST` no primeiro teste (passo 2). Depois apague a variável, ou use `PROD`. |

Segredos nunca vão para o repositório nem para arquivos do projeto. Só ficam no Environment
`release`.

### 1.4 `CODESIGNTOOL_SHA256`

O workflow baixa a ferramenta da SSL.com (CodeSignTool, versão fixa `v1.3.2`) e só a usa se o
SHA-256 do arquivo bater com o valor gravado no próprio workflow. A SSL.com não publica esse
número, então você o calcula uma vez no notebook. A partir daí, qualquer troca do arquivo no
servidor é recusada. Enquanto o valor estiver vazio, o job `sign` para antes de baixar qualquer
coisa.

1. No notebook Windows, baixe
   `https://github.com/SSLcom/CodeSignTool/releases/download/v1.3.2/CodeSignTool-v1.3.2-windows.zip`
   pelo navegador.
2. No PowerShell, calcule o hash:

   ```powershell
   Get-FileHash "$HOME\Downloads\CodeSignTool-v1.3.2-windows.zip" -Algorithm SHA256
   ```

3. Copie o valor da coluna `Hash`. No GitHub, abra o arquivo
   `.github/workflows/everydesk-release.yml` e clique no lápis (**Edit this file**). Cole o valor
   entre as aspas da linha `CODESIGNTOOL_SHA256: ""`. Ela fica assim, com o seu número:

   ```yaml
   CODESIGNTOOL_SHA256: "0123ABCD...64 caracteres..."
   ```

   Clique em **Commit changes**. Faça isso **antes** de criar a tag: a tag precisa apontar para
   um commit que já tenha o valor.

Se um dia trocar a versão (`CODESIGNTOOL_VERSION` no workflow), repita estes passos com o novo
arquivo.

## 2. Primeiro teste: sandbox do eSigner (`v0.1.0-rc1`)

O sandbox da SSL.com assina com um certificado **de teste**. Esse teste serve para provar que
o workflow inteiro funciona sem gastar a cota real.

1. Pegue as credenciais do sandbox. São as credenciais públicas de demonstração da SSL.com, em
   https://www.ssl.com/guide/esigner-demo-credentials-and-certificates/ , ou as da sua conta de
   sandbox. Coloque-as nos quatro segredos `ES_*`.
2. Crie a variável `ESIGNER_ENV` = `TEST` no Environment `release`.
3. No Mac, dentro da pasta `everydesk`, crie a tag:

   ```sh
   git tag v0.1.0-rc1
   ```

4. Envie a tag:

   ```sh
   git push origin v0.1.0-rc1
   ```

5. No GitHub, abra **Actions** → **EveryDesk release** → a execução da tag. Espere a
   compilação (job `build`, de 1 a 2 horas). Depois clique em **Review deployments** → marque
   `release` → **Approve and deploy**. A assinatura e a publicação levam uns 15 minutos.
6. Quando terminar, abra **Releases**. A `v0.1.0-rc1` aparece como **Pre-release**, com o
   aviso "Versão de TESTE".
7. No notebook, baixe `EveryDesk_0.1.0-rc1_x64-setup.exe` e `EveryDesk.exe` e confira as
   assinaturas (veja a seção 4). **No modo TEST o Windows diz que a raiz não é confiável. Isso
   é esperado**: o certificado do sandbox não é reconhecido pelo Windows. O que importa é que
   apareça uma assinatura da SSL.com e um carimbo de tempo.
8. Apague a release de teste em **Releases** → `v0.1.0-rc1` → ícone de lixeira. Depois apague a
   tag:

   ```sh
   git push --delete origin v0.1.0-rc1
   ```

   ```sh
   git tag -d v0.1.0-rc1
   ```

O workflow **recusa** `ESIGNER_ENV=TEST` em tag sem sufixo (`v0.1.0`). Assim, uma assinatura de
teste nunca chega aos usuários pelo updater.

## 3. Release de verdade

1. Troque os quatro segredos `ES_*` pelos dados da sua conta real (passo 1.2).
2. Apague a variável `ESIGNER_ENV`, ou mude o valor para `PROD`.
3. Confira `version` em `apps/everydesk/tauri.conf.json`. A tag precisa ter a mesma versão:
   `v0.1.0`, ou `v0.1.0-rcN` para uma pré-release da 0.1.0. Se não bater, o workflow para e
   avisa.
4. Faça primeiro uma pré-release assinada de verdade:

   ```sh
   git tag v0.1.0-rc2
   ```

   ```sh
   git push origin v0.1.0-rc2
   ```

5. Aprove em **Actions** (como no passo 2.5), baixe os arquivos e confira as assinaturas no
   notebook (seção 4). Agora `signtool verify /pa /v` tem que dizer
   `Successfully verified` nos dois arquivos.
6. Envie os dois arquivos ao VirusTotal e à Microsoft (seção 5).
7. Quando estiver tudo certo, publique a versão final:

   ```sh
   git tag v0.1.0
   ```

   ```sh
   git push origin v0.1.0
   ```

Para a próxima versão, mude `version` no `tauri.conf.json` (por exemplo `0.1.1`), faça commit e
push, e crie a tag `v0.1.1`.

## 4. Conferir as assinaturas no notebook

**Jeito simples, sem instalar nada.** Clique com o botão direito no arquivo → **Propriedades** →
aba **Assinaturas Digitais**. Deve aparecer o seu nome (o do certificado) e uma data e hora de
carimbo. Clique em **Detalhes**. Deve aparecer "Esta assinatura digital está OK".

**PowerShell, sem instalar nada:**

```powershell
Get-AuthenticodeSignature "$HOME\Downloads\EveryDesk.exe" | Format-List
```

```powershell
Get-AuthenticodeSignature "$HOME\Downloads\EveryDesk_0.1.0-rc2_x64-setup.exe" | Format-List
```

No release real, `Status` precisa ser `Valid`, `SignerCertificate` precisa trazer o seu nome e
`TimeStamperCertificate` não pode estar vazio.

**`signtool`** vem com o Windows SDK. Instale o SDK uma vez:

```powershell
winget install --id Microsoft.WindowsSDK.10.0.26100 -e
```

Depois, num PowerShell aberto na pasta Downloads:

```powershell
$signtool = (Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter signtool.exe | Where-Object { $_.Directory.Name -eq 'x64' } | Select-Object -Last 1).FullName
```

```powershell
& $signtool verify /pa /v .\EveryDesk.exe
```

```powershell
& $signtool verify /pa /v .\EveryDesk_0.1.0-rc2_x64-setup.exe
```

O resultado certo termina com `Successfully verified` e mostra `The signature is timestamped`.
No sandbox (passo 2), o esperado é um erro de raiz não confiável.

**Hash.** Confira se o arquivo baixado é o mesmo que o workflow publicou:

```powershell
Get-FileHash .\EveryDesk.exe -Algorithm SHA256
```

Compare o resultado com a linha de `EveryDesk.exe` no arquivo `SHA256SUMS` da release.

## 5. VirusTotal e Microsoft (alarme falso de antivírus)

Faça isto a cada versão nova, logo depois de publicar:

1. **VirusTotal:** abra https://www.virustotal.com e envie `EveryDesk.exe` e o setup. Alguns
   antivírus menores marcam todo programa de acesso remoto como "RemoteAdmin"/"PUA". Isso é
   comum (acontece com AnyDesk e RustDesk também). Anote quais marcaram.
2. **Microsoft (WDSI):** abra
   https://www.microsoft.com/en-us/wdsi/filesubmission , escolha **Software developer**, entre
   com a sua conta Microsoft e envie cada arquivo. Selecione "Incorrectly detected as malware"
   (ou "Microsoft Defender SmartScreen" se o problema for o aviso azul de "O Windows protegeu o
   computador"). Na descrição, escreva que é um programa de acesso remoto de código aberto
   (AGPL), assinado com o seu certificado, e coloque o link da release.
3. Se algum antivírus grande marcar o arquivo, use o formulário de "false positive" desse
   fabricante e informe o hash SHA-256 (de `SHA256SUMS`).

O aviso do SmartScreen some aos poucos, conforme mais pessoas baixam arquivos assinados pelo
mesmo certificado. Por isso o nome no certificado não deve mudar (seção 6).

## 6. Nome do publicador (decisão sua)

O Windows mostra como "Editor" o **nome que está no certificado**. Num certificado IV, esse é o
seu nome civil, do jeito que a SSL.com validou. Já o `publisher` do `tauri.conf.json`
(hoje `Rafael Fachini`, um valor provisório) aparece em **Configurações → Aplicativos**.

- Deixe os dois iguais. Se o nome validado no certificado sair diferente (por exemplo com o
  nome completo), troque `bundle.publisher` em `apps/everydesk/tauri.conf.json` para o mesmo
  texto antes do primeiro release real.
- Escolha o nome uma vez e não troque mais. A reputação no SmartScreen fica presa ao
  certificado. Trocar de nome (por exemplo para uma empresa, com um certificado OV e CNPJ)
  recomeça os avisos do zero.

## 7. Testar o updater com uma pré-release

Os EveryDesk instalados consultam
`https://github.com/rafaelfachini029-ship-it/everydesk/releases/latest/download/latest.json`.
O GitHub chama de "latest" só a release normal mais recente: **pré-releases são ignoradas**.
Escolhemos que tags `-rc` sejam pré-release para que um teste nunca chegue aos usuários por
acidente.

Enquanto só existirem pré-releases, o app não encontra atualização (fica quieto). Para testar o
updater de ponta a ponta antes da `v0.1.0`, quando ainda não há usuários:

1. Instale a `v0.1.0-rc2` no notebook.
2. Publique a `v0.1.0-rc3` (tag, como no passo 3.4).
3. Em **Releases** → `v0.1.0-rc3` → lápis (Edit), desmarque **Set as a pre-release**, marque
   **Set as the latest release** e salve. Em até 6 horas (ou ao reabrir o app, 30 s depois) o
   EveryDesk da rc2 mostra "Nova versão 0.1.0-rc3".
4. Depois do teste, volte a marcar **Set as a pre-release** e salve.

Depois que a `v0.1.0` existir, **não faça isso**: os usuários receberiam a versão de teste.

**Nunca marque como "latest" uma rc assinada no sandbox** (as que têm o aviso "Versão de TESTE").
O updater instalaria um programa com assinatura de teste, que o Windows não reconhece. Use só
uma rc assinada de verdade (passo 3) para este teste.

## 8. Se o workflow falhar

| Mensagem | O que fazer |
|---|---|
| `Falta o segredo ...` | Crie o segredo no Environment `release` (passo 1.3). |
| `CODESIGNTOOL_SHA256 está vazio` | Faça o passo 1.4 e crie a tag de novo, num commit que já tenha o valor. |
| `A tag vX não bate com version ...` | Ajuste `version` no `tauri.conf.json` ou crie a tag certa. |
| `ESIGNER_ENV=TEST (sandbox) só é permitido ...` | Apague `ESIGNER_ENV` (ou use `PROD`) para tags sem sufixo. |
| `plugins.updater.pubkey ... ainda é o marcador` | Faça o passo 1.1. |
| `CodeSignTool ... tem SHA-256 ..., diferente` | O arquivo da SSL.com mudou. Confira de novo (passo 1.4) antes de trocar o valor. |
| `CodeSignTool could not sign ...` | Usuário, senha, Credential ID ou código OTP errados, ou cota do eSigner esgotada. A mensagem da SSL.com aparece logo acima no log. |
| `... .sig does not verify with the updater public key` | `TAURI_SIGNING_PRIVATE_KEY` não é o par da chave pública colada no `tauri.conf.json`. |
| `signtool verify /pa falhou` | A assinatura não é confiável no Windows. Confira se não ficou `ESIGNER_ENV=TEST`. |

Para tentar de novo com a mesma tag, use **Re-run failed jobs** na execução do workflow, e
não **Re-run all jobs**. Cada vez que o job `sign` roda, ele gasta 9 assinaturas da cota do
eSigner. Se só o `publish` falhou, só ele roda de novo, sem assinar nada. Se a release chegou a
ser criada, apague-a antes.

Se você mudou um arquivo do repositório (por exemplo o passo 1.4), o re-run não vê a mudança,
porque ele usa o mesmo commit da tag. Nesse caso, apague a tag e crie-a de novo no commit novo:

```sh
git push --delete origin v0.1.0-rc1
```

```sh
git tag -d v0.1.0-rc1
```

```sh
git pull
```

```sh
git tag v0.1.0-rc1
```

```sh
git push origin v0.1.0-rc1
```

## 9. Instalador oficial

O **instalador oficial** é o setup NSIS (`EveryDesk_x.y.z_x64-setup.exe`). É ele que deve ser
indicado para quem vai deixar o EveryDesk instalado.

O botão **Instalar para acesso permanente** do portátil (`EveryDesk.exe`) também funciona e é
suportado. Ele pede permissão de administrador uma vez e usa o instalador do motor (o mesmo
`EveryDesk.exe`, com `--silent-install`):

- copia **só o `EveryDesk.exe`** para `C:\Program Files\EveryDesk` (nunca a pasta de onde o
  portátil foi aberto, como Downloads);
- cria o serviço do Windows e a inicialização automática;
- grava a mesma chave de desinstalação do setup NSIS
  (`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\EveryDesk`), então o EveryDesk
  aparece uma vez só em **Configurações → Aplicativos**.

Depois disso, o updater atualiza essa instalação normalmente: baixa o setup NSIS e o executa
com `/UPDATE`, na mesma pasta. O que esperar na primeira atualização: a entrada em
**Aplicativos** passa a ser a do setup NSIS (com o desinstalador `uninstall.exe`) e o serviço é
recriado pelos hooks do instalador. As configurações e os contatos continuam.

## 10. Contatos no modo serviço

Com o EveryDesk instalado (modo serviço), a lista de contatos de confiança fica no serviço, que
roda como SYSTEM. Ela vale **para o computador inteiro**, não para um usuário do Windows:

- um contato com acesso permanente (StayConnected) aceito por um usuário do Windows também
  consegue entrar enquanto **outro** usuário estiver conectado no computador;
- qualquer usuário conectado ao Windows pode alterar ou remover os contatos.

Desinstalar **não apaga** a lista guardada pelo serviço (SYSTEM). Para revogar tudo de vez,
remova os contatos no app **antes** de desinstalar.
