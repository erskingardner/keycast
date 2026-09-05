#!/usr/bin/env python3
"""Trusted-host backup/monitor jobs. Configuration contains paths and commands, never Nostr keys.
No jobs are installed or run remotely by this script unless explicitly invoked by the operator.
"""
import argparse
import datetime
import fcntl
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid


def command(config, name, default):
    value = config.get(name, default)
    if not isinstance(value, list) or not value or not all(isinstance(x, str) for x in value):
        raise ValueError(f"{name} must be an argument array")
    return value


def run(argv, timeout=300):
    # Never use a shell, echo argv, or include subprocess output in public monitoring errors.
    return subprocess.run(argv, check=True, capture_output=True, text=True, timeout=timeout)


def atomic_stamp(path):
    temporary = path.with_name(".stamp-" + uuid.uuid4().hex)
    try:
        with temporary.open("x") as output:
            json.dump({"completed_at": int(time.time())}, output)
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(path)
        descriptor = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    finally:
        temporary.unlink(missing_ok=True)


def backup(config):
    directory = Path(config["backup_directory"])
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    with (directory / ".backup-job.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return backup_locked(config, directory)


def backup_locked(config, directory):
    # Require an explicitly configured off-host destination before producing/pruning backups.
    remote = config["remote_directory"]
    if not isinstance(remote, str) or not remote or ":" not in remote:
        raise ValueError("configure an rclone off-host remote_directory")
    keep = int(config.get("keep_local", 7))
    if keep < 2:
        raise ValueError("keep_local must retain at least two successful backups")
    stamp = directory / "last-offhost-success.json"
    name = "keycast-" + datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ-") + uuid.uuid4().hex + ".kcb"
    archive = directory / name
    signer = command(config, "signer_command", ["keycast_signer"])
    rclone = command(config, "rclone_command", ["rclone"])
    run(signer + ["backup", config["backup_key_file"], str(archive)], 1800)
    # Completion means upload and a byte-for-byte readback succeeded. A provider ACK alone is
    # insufficient. This uses download verification without creating another local plaintext file.
    target = remote.rstrip("/") + "/" + name
    run(rclone + ["copyto", str(archive), target], 1800)
    run(rclone + ["check", str(directory), remote, "--include", "/" + name, "--one-way", "--download"], 1800)
    atomic_stamp(stamp)
    # Only prune owned local archives after off-host verification. Remote retention is deliberately
    # delegated to the operator's versioned/immutable bucket policy; never delete remote backups here.
    archives = sorted((p for p in directory.glob("keycast-*.kcb") if p.is_file() and not p.is_symlink()), key=lambda p: p.stat().st_mtime, reverse=True)
    for old in archives[keep:]:
        old.unlink()
    return {"ok": True, "offhost_verified_at": int(time.time())}


def evaluate(status, free_bytes, stamp, now, config):
    alerts = []
    resource = status["resources"]
    if not status["ready"]:
        alerts.append("signer_not_ready")
    if resource["inbox_records"] >= 8000 or resource["inbox_bytes"] >= 100 * 1024 * 1024:
        alerts.append("inbox_capacity")
    if resource["oldest_response_age_seconds"] > 60:
        alerts.append("outbox_age")
    if resource["database_bytes"] >= 200 * 1024 * 1024:
        alerts.append("database_capacity")
    if resource["wal_bytes"] >= 64 * 1024 * 1024:
        alerts.append("wal_growth")
    if free_bytes < int(config.get("minimum_free_bytes", 1024 * 1024 * 1024)):
        alerts.append("disk_space")
    if stamp is None or now - stamp > int(config.get("maximum_backup_age_seconds", 90000)) or stamp > now + 60:
        alerts.append("offhost_backup_age")
    return {"ok": not alerts, "alerts": alerts}


def monitor(config):
    status = json.loads(run(command(config, "status_command", ["keycast_signer"]) + ["status"], 30).stdout)
    stamp = None
    try:
        stamp = json.loads((Path(config["backup_directory"]) / "last-offhost-success.json").read_text())["completed_at"]
    except (OSError, ValueError, KeyError):
        pass
    free = shutil.disk_usage(config["database_directory"]).free
    return evaluate(status, free, stamp, time.time(), config)


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["backup", "monitor"])
    parser.add_argument("config", type=Path)
    args = parser.parse_args()
    try:
        config = json.loads(args.config.read_text())
        result = backup(config) if args.operation == "backup" else monitor(config)
    except Exception as error:
        result = {"ok": False, "alerts": ["operation_failed"], "error_type": type(error).__name__}
    print(json.dumps(result))
    return 0 if result["ok"] else 2


if __name__ == "__main__":
    sys.exit(main())
