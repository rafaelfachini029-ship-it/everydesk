#!/usr/bin/env python3
"""Writes the updater manifest (latest.json) for an EveryDesk release.

Used by .github/workflows/everydesk-release.yml after both Windows files are signed
(Authenticode first, then `tauri signer sign`). Standard library only, so it runs on any
runner without pip.

The manifest has two targets, matching src/update.rs:
  windows-x86_64            -> the NSIS setup (installed copies)
  windows-x86_64-portable   -> the portable EveryDesk.exe
Each `signature` is the content of the matching `.sig` file (base64 of a minisign
signature box), exactly what the Tauri updater expects.

Before writing, every signature is checked:
  * it decodes as a minisign signature;
  * its trusted comment carries `version:<version>` (the app uses requireSignedVersion);
  * when a real updater public key is given (--pubkey-from-conf or --pubkey), the
    signature verifies against the file bytes with that key. A key mismatch here would
    publish an update that every installed copy refuses, so it fails the release.

Example:
  python make_latest_json.py --version 0.1.0 \
    --base-url https://github.com/OWNER/REPO/releases/download/v0.1.0 \
    --setup target/release/bundle/nsis/EveryDesk_0.1.0_x64-setup.exe \
    --portable dist/EveryDesk.exe \
    --pubkey-from-conf apps/everydesk/tauri.conf.json \
    --notes "EveryDesk 0.1.0" --out dist/latest.json
"""

from __future__ import annotations

import argparse
import base64
import binascii
import datetime as _dt
import hashlib
import json
import os
import re
import sys

SETUP_TARGET = "windows-x86_64"
PORTABLE_TARGET = "windows-x86_64-portable"
PUBKEY_PLACEHOLDER = "COLE_AQUI_A_CHAVE_PUBLICA_DO_UPDATER"

# Semver as the updater parses it (an optional leading "v" is accepted by the app, but the
# manifest always carries the bare version).
_SEMVER = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


class ManifestError(Exception):
    pass


# --------------------------------------------------------------------------------------
# Ed25519 verification (RFC 8032, section 5.1.7). Pure Python, verification only; inputs are
# public (release files, public key, signatures), so constant time does not matter here.
# --------------------------------------------------------------------------------------
_P = 2**255 - 19
_L = 2**252 + 27742317777372353535851937790883648493
_D = (-121665 * pow(121666, _P - 2, _P)) % _P
_SQRT_M1 = pow(2, (_P - 1) // 4, _P)


def _edwards_add(p1, p2):
    x1, y1, z1, t1 = p1
    x2, y2, z2, t2 = p2
    a = (y1 - x1) * (y2 - x2) % _P
    b = (y1 + x1) * (y2 + x2) % _P
    c = 2 * t1 * t2 * _D % _P
    d = 2 * z1 * z2 % _P
    e, f, g, h = b - a, d - c, d + c, b + a
    return (e * f % _P, g * h % _P, f * g % _P, e * h % _P)


def _scalar_mult(s, point):
    q = (0, 1, 1, 0)
    while s > 0:
        if s & 1:
            q = _edwards_add(q, point)
        point = _edwards_add(point, point)
        s >>= 1
    return q


def _point_equal(p1, p2):
    x1, y1, z1, _ = p1
    x2, y2, z2, _ = p2
    return (x1 * z2 - x2 * z1) % _P == 0 and (y1 * z2 - y2 * z1) % _P == 0


def _recover_x(y, sign):
    if y >= _P:
        return None
    x2 = (y * y - 1) * pow(_D * y * y + 1, _P - 2, _P)
    if x2 == 0:
        return None if sign else 0
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P != 0:
        x = x * _SQRT_M1 % _P
    if (x * x - x2) % _P != 0:
        return None
    if (x & 1) != sign:
        x = _P - x
    return x


def _point_decompress(s: bytes):
    if len(s) != 32:
        return None
    y = int.from_bytes(s, "little")
    sign = y >> 255
    y &= (1 << 255) - 1
    x = _recover_x(y, sign)
    if x is None:
        return None
    return (x, y, 1, x * y % _P)


_G_Y = 4 * pow(5, _P - 2, _P) % _P
_G = (_recover_x(_G_Y, 0), _G_Y, 1, _recover_x(_G_Y, 0) * _G_Y % _P)


def ed25519_verify(public: bytes, msg: bytes, signature: bytes) -> bool:
    if len(public) != 32 or len(signature) != 64:
        return False
    a = _point_decompress(public)
    if a is None:
        return False
    r_bytes = signature[:32]
    r = _point_decompress(r_bytes)
    if r is None:
        return False
    s = int.from_bytes(signature[32:], "little")
    if s >= _L:
        return False
    h = int.from_bytes(hashlib.sha512(r_bytes + public + msg).digest(), "little") % _L
    return _point_equal(_scalar_mult(s, _G), _edwards_add(r, _scalar_mult(h, a)))


# --------------------------------------------------------------------------------------
# minisign formats as Tauri writes them
# --------------------------------------------------------------------------------------
def _b64decode(text: str, what: str) -> bytes:
    try:
        return base64.b64decode(text.strip(), validate=True)
    except (binascii.Error, ValueError) as e:
        raise ManifestError(f"{what}: not valid base64 ({e})") from None


def decode_pubkey(pubkey_b64: str) -> tuple[bytes, bytes]:
    """tauri.conf.json `plugins.updater.pubkey` -> (key id, 32-byte Ed25519 key)."""
    text = _b64decode(pubkey_b64, "updater pubkey").decode("utf-8", "replace")
    lines = [ln for ln in text.splitlines() if ln.strip()]
    if len(lines) < 2 or not lines[0].startswith("untrusted comment:"):
        raise ManifestError("updater pubkey: not a minisign public key")
    raw = _b64decode(lines[1], "updater pubkey")
    if len(raw) != 42 or raw[:2] != b"Ed":
        raise ManifestError("updater pubkey: unexpected minisign key format")
    return raw[2:10], raw[10:]


class Signature:
    def __init__(self, sig_file_text: str, what: str):
        text = _b64decode(sig_file_text, what).decode("utf-8", "replace")
        lines = text.splitlines()
        if (
            len(lines) < 4
            or not lines[0].startswith("untrusted comment:")
            or not lines[2].startswith("trusted comment: ")
        ):
            raise ManifestError(f"{what}: not a minisign signature")
        raw = _b64decode(lines[1], what)
        if len(raw) != 74 or raw[:2] not in (b"Ed", b"ED"):
            raise ManifestError(f"{what}: unexpected minisign signature format")
        self.prehashed = raw[:2] == b"ED"
        self.key_id = raw[2:10]
        self.sig = raw[10:]
        self.trusted_comment = lines[2][len("trusted comment: "):]
        self.global_sig = _b64decode(lines[3], what)
        if len(self.global_sig) != 64:
            raise ManifestError(f"{what}: bad global signature")

    def field(self, name: str) -> str | None:
        for part in self.trusted_comment.split("\t"):
            if part.startswith(name + ":"):
                return part[len(name) + 1:]
        return None

    def verify(self, data: bytes, key_id: bytes, public: bytes) -> bool:
        if key_id != self.key_id:
            return False
        msg = hashlib.blake2b(data, digest_size=64).digest() if self.prehashed else data
        if not ed25519_verify(public, msg, self.sig):
            return False
        return ed25519_verify(public, self.sig + self.trusted_comment.encode("utf-8"), self.global_sig)


def check_signature(path: str, sig_text: str, version: str, pubkey_b64: str | None) -> None:
    """Raises ManifestError unless `sig_text` is a usable updater signature of `path`."""
    name = os.path.basename(path)
    sig = Signature(sig_text, f"{name}.sig")
    signed = sig.field("version")
    if signed is None:
        raise ManifestError(
            f"{name}.sig has no version in its trusted comment "
            "(sign with `cargo tauri signer sign --app-version`); the app would refuse it"
        )
    if signed.lstrip("v") != version.lstrip("v"):
        raise ManifestError(f"{name}.sig is bound to version {signed}, not {version}")
    signed_file = sig.field("file")
    if signed_file is not None and signed_file != name:
        print(f"warning: {name}.sig was made for file name {signed_file!r}", file=sys.stderr)
    if pubkey_b64 is None:
        return
    key_id, public = decode_pubkey(pubkey_b64)
    with open(path, "rb") as f:
        data = f.read()
    if not sig.verify(data, key_id, public):
        raise ManifestError(
            f"{name}.sig does not verify with the updater public key in tauri.conf.json "
            "(wrong TAURI_SIGNING_PRIVATE_KEY, or the file changed after `tauri signer sign`)"
        )


# --------------------------------------------------------------------------------------
# manifest
# --------------------------------------------------------------------------------------
def build_manifest(
    version: str,
    notes: str,
    pub_date: str,
    base_url: str,
    setup_name: str,
    setup_sig: str,
    portable_name: str,
    portable_sig: str,
) -> dict:
    if not _SEMVER.match(version):
        raise ManifestError(f"version {version!r} is not semver (x.y.z or x.y.z-rc1)")
    if not base_url.startswith("https://"):
        raise ManifestError("base url must be https")
    try:
        _dt.datetime.strptime(pub_date, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        raise ManifestError(f"pub_date {pub_date!r} must be RFC 3339 UTC, e.g. 2026-10-20T12:00:00Z") from None
    base = base_url.rstrip("/")
    for sig in (setup_sig, portable_sig):
        if not sig.strip():
            raise ManifestError("empty signature")
    return {
        "version": version,
        "notes": notes,
        "pub_date": pub_date,
        "platforms": {
            SETUP_TARGET: {"url": f"{base}/{setup_name}", "signature": setup_sig.strip()},
            PORTABLE_TARGET: {"url": f"{base}/{portable_name}", "signature": portable_sig.strip()},
        },
    }


def read_conf_pubkey(conf_path: str) -> str | None:
    """Returns the configured updater key, or None while it is still the placeholder."""
    with open(conf_path, encoding="utf-8") as f:
        conf = json.load(f)
    key = conf.get("plugins", {}).get("updater", {}).get("pubkey", "")
    if not key or key == PUBKEY_PLACEHOLDER:
        return None
    return key


def _read_sig(path: str) -> str:
    sig_path = path + ".sig"
    if not os.path.isfile(sig_path):
        raise ManifestError(f"missing {sig_path}")
    with open(sig_path, encoding="utf-8") as f:
        return f.read().strip()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--version", required=True)
    ap.add_argument("--base-url", required=True, help="https://github.com/OWNER/REPO/releases/download/TAG")
    ap.add_argument("--setup", required=True, help="path of the signed NSIS setup (its .sig must sit next to it)")
    ap.add_argument("--portable", required=True, help="path of the signed portable exe (its .sig must sit next to it)")
    key = ap.add_mutually_exclusive_group()
    key.add_argument("--pubkey-from-conf", help="tauri.conf.json; verifies both signatures with its updater key")
    key.add_argument("--pubkey", help="updater public key (base64, as in tauri.conf.json)")
    ap.add_argument("--require-pubkey", action="store_true", help="fail when the configured key is the placeholder")
    ap.add_argument("--notes", default=None)
    ap.add_argument("--pub-date", default=None, help="default: now, UTC")
    ap.add_argument("--out", required=True)
    args = ap.parse_args(argv)

    try:
        version = args.version.lstrip("v")
        pubkey = args.pubkey
        if args.pubkey_from_conf:
            pubkey = read_conf_pubkey(args.pubkey_from_conf)
        if pubkey is None:
            if args.require_pubkey:
                raise ManifestError(
                    "tauri.conf.json still has the updater pubkey placeholder; see docs/release.md"
                )
            print("warning: no updater public key, signatures not verified", file=sys.stderr)
        sigs = {}
        for path in (args.setup, args.portable):
            if not os.path.isfile(path):
                raise ManifestError(f"missing {path}")
            sigs[path] = _read_sig(path)
            check_signature(path, sigs[path], version, pubkey)
        pub_date = args.pub_date or _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        manifest = build_manifest(
            version=version,
            notes=args.notes if args.notes is not None else f"EveryDesk {version}",
            pub_date=pub_date,
            base_url=args.base_url,
            setup_name=os.path.basename(args.setup),
            setup_sig=sigs[args.setup],
            portable_name=os.path.basename(args.portable),
            portable_sig=sigs[args.portable],
        )
    except ManifestError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1

    with open(args.out, "w", encoding="utf-8", newline="\n") as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"wrote {args.out} for {version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
