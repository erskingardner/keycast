# Host operation units

These are templates. Review every path before enabling them.

`operations.example.json` assumes the signer is a **host binary** at
`/usr/local/bin/keycast_signer`. Keep it that way if you can: pointing
`signer_command` at `docker compose run ...` requires `User=keycast` to be in the
`docker` group, which is root-equivalent on the host and defeats every sandboxing
directive in these units.

Both units are sandboxed with `ProtectSystem=strict`, an empty capability
bounding set, a `@system-service` syscall filter, and write access only to the
specific paths each job needs:

- backup: writes the encrypted archive and the transient snapshot, so it needs
  `/srv/keycast/database` and `/srv/keycast/backups`.
- monitor: writes nothing; it needs `/run/keycast` only because connecting to a
  Unix socket requires write permission on the socket inode.

Adjust `ReadWritePaths=`, `ReadOnlyPaths=` and `backup_key_file` to your layout.
Verify the result with:

```sh
systemd-analyze security keycast-backup.service
systemd-analyze security keycast-monitor.service
```

Configure `OnFailure=` on both units to reach your alert receiver before you rely
on them. A backup job that fails silently is not a backup.
