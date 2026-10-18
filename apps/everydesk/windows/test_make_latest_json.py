"""Tests for make_latest_json.py.

Run from the repo root:
  python3 -I apps/everydesk/windows/test_make_latest_json.py

testdata/ holds a throwaway key pair's PUBLIC keys (updater.pub, other.pub) and signatures made
with the real `cargo tauri signer sign` (tauri-cli 2.12.3), so the minisign checks run against
what the release workflow actually produces. The private test key is not kept anywhere.
"""

import base64
import json
import os
import re
import shutil
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import make_latest_json as m  # noqa: E402

DATA = os.path.join(HERE, "testdata")
UPDATE_RS = os.path.join(HERE, "..", "src", "update.rs")
BASE = "https://github.com/rafaelfachini029-ship-it/everydesk/releases/download/v0.2.0"


def read(name):
    with open(os.path.join(DATA, name), encoding="utf-8") as f:
        return f.read().strip()


def rust_fixture():
    """The MANIFEST constant from src/update.rs (the shape the app's parser test pins)."""
    with open(UPDATE_RS, encoding="utf-8") as f:
        src = f.read()
    found = re.search(r'const MANIFEST: &str = r#"(.*?)"#;', src, re.S)
    assert found, "MANIFEST fixture not found in update.rs"
    return json.loads(found.group(1))


class ManifestShape(unittest.TestCase):
    def test_matches_the_fixture_the_app_parses(self):
        want = rust_fixture()
        got = m.build_manifest(
            version="0.2.0",
            notes="EveryDesk 0.2.0",
            pub_date="2026-10-20T12:00:00Z",
            base_url=BASE + "/",
            setup_name="EveryDesk_0.2.0_x64-setup.exe",
            setup_sig="c2V0dXA=",
            portable_name="EveryDesk.exe",
            portable_sig="cG9ydGFibGU=\n",
        )
        self.assertEqual(got, want)
        self.assertEqual(set(got["platforms"]), {m.SETUP_TARGET, m.PORTABLE_TARGET})

    def test_rejects_bad_inputs(self):
        ok = dict(
            version="0.2.0", notes="n", pub_date="2026-10-20T12:00:00Z", base_url=BASE,
            setup_name="s.exe", setup_sig="c2V0dXA=", portable_name="p.exe", portable_sig="cA==",
        )
        m.build_manifest(**ok)
        m.build_manifest(**{**ok, "version": "0.1.0-rc1"})
        for bad in (
            {"version": "v0.2.0"},
            {"version": "0.2"},
            {"version": ""},
            {"pub_date": "2026-10-20 12:00"},
            {"base_url": "http://example.com"},
            {"setup_sig": " "},
            {"portable_sig": ""},
        ):
            with self.assertRaises(m.ManifestError, msg=bad):
                m.build_manifest(**{**ok, **bad})


class Signatures(unittest.TestCase):
    def setUp(self):
        self.pub = read("updater.pub")

    def test_real_tauri_signatures_verify(self):
        for f in ("portable.bin", "setup.bin"):
            m.check_signature(os.path.join(DATA, f), read(f + ".sig"), "0.2.0", self.pub)
            m.check_signature(os.path.join(DATA, f), read(f + ".sig"), "v0.2.0", self.pub)

    def test_without_a_key_only_the_version_is_checked(self):
        m.check_signature(os.path.join(DATA, "portable.bin"), read("portable.bin.sig"), "0.2.0", None)

    def test_wrong_version_is_refused(self):
        with self.assertRaisesRegex(m.ManifestError, "bound to version 0.2.0"):
            m.check_signature(os.path.join(DATA, "portable.bin"), read("portable.bin.sig"), "0.3.0", None)

    def test_signature_without_version_is_refused(self):
        with self.assertRaisesRegex(m.ManifestError, "no version"):
            m.check_signature(os.path.join(DATA, "portable.bin"), read("noversion.sig"), "0.2.0", self.pub)

    def test_other_key_is_refused(self):
        with self.assertRaisesRegex(m.ManifestError, "does not verify"):
            m.check_signature(os.path.join(DATA, "portable.bin"), read("portable.bin.sig"), "0.2.0", read("other.pub"))

    def test_signature_of_another_file_is_refused(self):
        with self.assertRaisesRegex(m.ManifestError, "does not verify"):
            m.check_signature(os.path.join(DATA, "setup.bin"), read("portable.bin.sig"), "0.2.0", self.pub)

    def test_changed_bytes_are_refused(self):
        tmp = tempfile.mkdtemp()
        try:
            path = os.path.join(tmp, "portable.bin")
            with open(os.path.join(DATA, "portable.bin"), "rb") as f:
                data = bytearray(f.read())
            data[-2] ^= 1
            with open(path, "wb") as f:
                f.write(data)
            with self.assertRaisesRegex(m.ManifestError, "does not verify"):
                m.check_signature(path, read("portable.bin.sig"), "0.2.0", self.pub)
        finally:
            shutil.rmtree(tmp)

    def test_tampered_trusted_comment_is_refused(self):
        text = base64.b64decode(read("portable.bin.sig")).decode()
        forged = base64.b64encode(text.replace("version:0.2.0", "version:0.3.0").encode()).decode()
        with self.assertRaisesRegex(m.ManifestError, "does not verify"):
            m.check_signature(os.path.join(DATA, "portable.bin"), forged, "0.3.0", self.pub)

    def test_garbage_is_refused(self):
        for junk in ("", "not base64!", base64.b64encode(b"hello").decode()):
            with self.assertRaises(m.ManifestError):
                m.check_signature(os.path.join(DATA, "portable.bin"), junk, "0.2.0", None)
        with self.assertRaises(m.ManifestError):
            m.decode_pubkey(m.PUBKEY_PLACEHOLDER)


class CommandLine(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        for f in ("portable.bin", "portable.bin.sig", "setup.bin", "setup.bin.sig"):
            shutil.copy(os.path.join(DATA, f), self.tmp)
        self.conf = os.path.join(self.tmp, "tauri.conf.json")
        self.write_conf(read("updater.pub"))
        self.out = os.path.join(self.tmp, "latest.json")

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def write_conf(self, key):
        with open(self.conf, "w", encoding="utf-8") as f:
            json.dump({"plugins": {"updater": {"pubkey": key}}}, f)

    def run_main(self, *extra):
        return m.main([
            "--version", "v0.2.0", "--base-url", BASE,
            "--setup", os.path.join(self.tmp, "setup.bin"),
            "--portable", os.path.join(self.tmp, "portable.bin"),
            "--pub-date", "2026-10-20T12:00:00Z", "--out", self.out, *extra,
        ])

    def test_writes_the_manifest(self):
        self.assertEqual(self.run_main("--pubkey-from-conf", self.conf, "--require-pubkey"), 0)
        with open(self.out, encoding="utf-8") as f:
            got = json.load(f)
        self.assertEqual(got["version"], "0.2.0")
        self.assertEqual(got["notes"], "EveryDesk 0.2.0")
        self.assertEqual(got["platforms"][m.PORTABLE_TARGET]["url"], BASE + "/portable.bin")
        self.assertEqual(got["platforms"][m.PORTABLE_TARGET]["signature"], read("portable.bin.sig"))
        self.assertEqual(got["platforms"][m.SETUP_TARGET]["signature"], read("setup.bin.sig"))

    def test_placeholder_key(self):
        self.write_conf(m.PUBKEY_PLACEHOLDER)
        self.assertEqual(self.run_main("--pubkey-from-conf", self.conf, "--require-pubkey"), 1)
        self.assertFalse(os.path.exists(self.out))
        self.assertEqual(self.run_main("--pubkey-from-conf", self.conf), 0)

    def test_wrong_key_fails(self):
        self.write_conf(read("other.pub"))
        self.assertEqual(self.run_main("--pubkey-from-conf", self.conf), 1)
        self.assertFalse(os.path.exists(self.out))

    def test_missing_sig_fails(self):
        os.remove(os.path.join(self.tmp, "setup.bin.sig"))
        self.assertEqual(self.run_main(), 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
