import os
import tempfile
import unittest
from pathlib import Path

from tools.agent_sandbox.launch_web_server import (
    bootstrap_sandbox_env,
    parse_env_vars,
    resolve_http_port,
    resolve_sandbox_env,
    select_ank_server_binary,
)


class LaunchWebServerTests(unittest.TestCase):
    def test_bootstrap_sandbox_env_isolates_ports_and_carries_root_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                'OPENROUTER_API_KEYS=sk-or-fake-1\nGEMINI_API_KEYS="g-fake-2"\nUNRELATED=ignored\n',
                encoding="utf-8",
            )
            target = root / "tools" / "agent_sandbox" / ".env"
            created = bootstrap_sandbox_env(root, target)
            self.assertEqual(created, target)
            parsed = parse_env_vars(created)

            self.assertEqual(parsed["AEGIS_HTTP_PORT"], "8090")
            self.assertEqual(parsed["ANK_HTTP_PORT"], "8090")
            self.assertEqual(parsed["AEGIS_GRPC_PORT"], "50059")
            self.assertEqual(parsed["OPENROUTER_API_KEYS"], "sk-or-fake-1")
            self.assertEqual(parsed["GEMINI_API_KEYS"], "g-fake-2")
            self.assertNotIn("UNRELATED", parsed)
            self.assertEqual(len(parsed["AEGIS_ROOT_KEY"]), 64)
            self.assertTrue(parsed["AEGIS_DATA_DIR"].endswith("tools/agent_sandbox/server_data"))
            self.assertEqual(resolve_http_port(created), 8090)

    def test_resolve_sandbox_env_prefers_explicit_then_local_then_bootstrap(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            explicit = root / "explicit.env"
            explicit.write_text("AEGIS_HTTP_PORT=8091\n", encoding="utf-8")

            # Explicit AEGIS_SANDBOX_ENV path takes precedence when present
            chosen = resolve_sandbox_env(repo_root=root, explicit_env=str(explicit))
            self.assertEqual(chosen, explicit)
            self.assertEqual(resolve_http_port(chosen), 8091)

            # Repo-local tools/agent_sandbox/.env is used when explicit is omitted
            local_env = root / "tools" / "agent_sandbox" / ".env"
            local_env.parent.mkdir(parents=True, exist_ok=True)
            local_env.write_text("AEGIS_HTTP_PORT=8095\n", encoding="utf-8")
            chosen_local = resolve_sandbox_env(repo_root=root)
            self.assertEqual(chosen_local, local_env)
            self.assertEqual(resolve_http_port(chosen_local), 8095)

            # Bootstraps local .env automatically if neither explicit nor local exists
            local_env.unlink()
            bootstrapped = resolve_sandbox_env(repo_root=root)
            self.assertEqual(bootstrapped, local_env)
            self.assertEqual(resolve_http_port(bootstrapped), 8090)

    def test_select_ank_server_binary_prefers_newest_mtime(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            ext = ".exe" if os.name == "nt" else ""
            debug_bin = root / "target" / "debug" / f"ank-server{ext}"
            release_bin = root / "target" / "release" / f"ank-server{ext}"
            debug_bin.parent.mkdir(parents=True, exist_ok=True)
            release_bin.parent.mkdir(parents=True, exist_ok=True)
            release_bin.write_bytes(b"rel")
            debug_bin.write_bytes(b"dbg")
            os.utime(release_bin, (1000, 1000))
            os.utime(debug_bin, (2000, 2000))

            self.assertEqual(select_ank_server_binary(root), debug_bin)


if __name__ == "__main__":
    unittest.main()
