# Vetor de teste dos envelopes EveryDesk (lado do app)

Este é o mesmo vetor do servidor (`everydesk-server/docs/protocolo-everydesk-vector.md`, teste
`known_answer_vector`). Do lado do app, o teste é
`src/everydesk_auth.rs::tests::envelopes_known_answer`. Ele confere byte a byte que, com as
mesmas entradas, o cliente gera os mesmos envelopes que o servidor.

Formato do envelope: `<server_key>;v1;<my_id>;<ts>;<base64(sig)>`.
- `sig` é a assinatura ed25519 destacada da mensagem, em UTF-8.
- A base64 é a padrão, com `=`.
- Mensagem de punch/relay (`PunchHoleRequest.licence_key`, `RequestRelay.licence_key`):
  `ed-punch-v1|<my_id>|<target_id>|<ts>`.
- Mensagem de presença (`OnlineRequest.id`): `ed-online-v1|<my_id>|<ts>`.

## Entradas

| campo | valor |
|---|---|
| seed ed25519 (hex) | `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f` |
| server_key | `SRVKEY` |
| my_id | `123456789` |
| target_id | `987654321` |
| ts | `1800000000` |

## Saídas

Chave pública (pk), em base64:

```
A6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg=
```

Envelope de punch/relay:

```
SRVKEY;v1;123456789;1800000000;YJ7AacQGlO9/sFaHhtKSUDdjP5YQrSTDP9k9T7RPIQhAFTsqNfnlwMO1ifWwcnJuUxWEakavEgRpQtYhzbrNBA==
```

Envelope de presença:

```
SRVKEY;v1;123456789;1800000000;K4o10Ltj36N69t9EPu7drFemTzQtDvuqvoQ1ijCniecHoVbOiG2HWH+u06GbF0EqIkIgNSy+BhA8Wi69QS/6Ag==
```

## Relógio

O servidor só aceita `ts` dentro de ±120 s do relógio dele.

Se a assinatura for válida mas o `ts` estiver fora dessa janela, o servidor responde
`PunchHoleResponse.other_failure = "clock"`. O cliente então mostra:

> O relógio deste computador está errado. Acerte a data e a hora e tente de novo.

Nesse caso o cliente não tenta reconectar sozinho.

Quando não há resposta nenhuma (punch ou relay), o erro genérico ganha a dica:

> Se o problema persistir, confira o relógio do computador.
