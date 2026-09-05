import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import keycast_ops as ops


class OperationsTests(unittest.TestCase):
    def test_monitor_reports_independent_capacity_readiness_and_backup_failures(self):
        status = {"ready": False, "resources": {"inbox_records": 9000, "inbox_bytes": 0, "oldest_response_age_seconds": 90, "database_bytes": 210 * 1024**2, "wal_bytes": 70 * 1024**2}}
        result = ops.evaluate(status, 0, None, 1000, {})
        self.assertEqual(set(result["alerts"]), {"signer_not_ready", "inbox_capacity", "outbox_age", "database_capacity", "wal_growth", "disk_space", "offhost_backup_age"})
        status["ready"] = True
        status["resources"] = dict.fromkeys(status["resources"], 0)
        self.assertTrue(ops.evaluate(status, 2 * 1024**3, 900, 1000, {})["ok"])
        self.assertFalse(ops.evaluate(status, 2 * 1024**3, 2000, 1000, {})["ok"])

    def test_upload_failure_preserves_old_backups_and_success_stamp(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            for n in range(4): (directory / f"keycast-{n}.kcb").write_text("old")
            stamp = directory / "last-offhost-success.json"
            stamp.write_text('{"completed_at":123}')
            config = {"backup_directory": tmp, "backup_key_file": "/not-read/test.key", "remote_directory": "test:offhost", "keep_local": 2}
            with patch.object(ops, "run", side_effect=[None, RuntimeError("upload failed")]):
                with self.assertRaises(RuntimeError): ops.backup(config)
            self.assertEqual(json.loads(stamp.read_text())["completed_at"], 123)
            self.assertEqual(len(list(directory.glob("*.kcb"))), 4)

    def test_verified_upload_precedes_success_and_local_retention(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            for n in range(4): (directory / f"keycast-{n}.kcb").write_text("old")
            config = {"backup_directory": tmp, "backup_key_file": "/not-read/test.key", "remote_directory": "test:offhost", "keep_local": 2}
            with patch.object(ops, "run") as run:
                self.assertTrue(ops.backup(config)["ok"])
            self.assertIn("copyto", run.call_args_list[1].args[0])
            self.assertIn("--download", run.call_args_list[2].args[0])
            self.assertTrue((directory / "last-offhost-success.json").exists())
            self.assertEqual(len(list(directory.glob("*.kcb"))), 2)


if __name__ == "__main__": unittest.main()
