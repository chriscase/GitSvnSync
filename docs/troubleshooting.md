# Troubleshooting

## Common Issues

### Daemon won't start

**Symptom**: `systemctl start gitsvnsync` fails

**Check logs**:
```bash
journalctl -u gitsvnsync -n 50 --no-pager
```

**Common causes**:
- Config file not found → verify path in service file
- Missing environment variables → check `/etc/gitsvnsync/env`
- Port already in use → check `ss -tlnp | grep 8080`
- Database permissions → verify `gitsvnsync` user owns data directory

### SVN authentication failure

**Symptom**: Logs show "SVN authentication failed"

**Debug**:
```bash
# Test SVN credentials manually
svn info --username sync-service --password "$GITSVNSYNC_SVN_PASSWORD" https://svn.company.com/repos/project
```

**Common causes**:
- Wrong password in env file
- SVN URL incorrect (check for trailing slash)
- Network/firewall blocking access to SVN server

### GitHub authentication failure

**Symptom**: Logs show "GitHub API returned 401"

**Debug**:
```bash
# Test GitHub token manually
curl -H "Authorization: token $GITSVNSYNC_GITHUB_TOKEN" https://api.github.com/user
# For GHE:
curl -H "Authorization: token $GITSVNSYNC_GITHUB_TOKEN" https://github.company.com/api/v3/user
```

**Common causes**:
- Token expired or revoked
- Token missing `repo` scope
- Wrong API URL (GitHub.com vs GHE)

### Author mapping not working

**Symptom**: Commits appear as wrong author or as the sync service account

**Debug**:
```bash
gitsvnsync identity list
```

**Common causes**:
- User not in authors.toml → add the mapping
- SVN pre-revprop-change hook not enabled → see getting-started.md
- Typo in SVN username in mapping file

### Commits not syncing

**Symptom**: Changes made on one side don't appear on the other

**Check sync status**:
```bash
gitsvnsync status
gitsvnsync audit --limit 10
```

**Common causes**:
- Daemon not running → `systemctl status gitsvnsync`
- Sync is paused due to unresolved conflict → `gitsvnsync conflicts list`
- Webhook not configured (relying on polling) → check poll interval
- Echo suppression false positive → check commit mapping table

### Webhook not received

**Symptom**: Changes sync only on poll interval, not immediately

**Debug**:
- Verify webhook is configured in GitHub repo settings
- Check webhook delivery log in GitHub (Settings → Webhooks → Recent Deliveries)
- Verify daemon is accessible from GitHub (firewall, DNS)
- Check webhook secret matches

### Database corruption

**Symptom**: "database disk image is malformed" error

**Recovery**:
```bash
# Stop daemon
sudo systemctl stop gitsvnsync

# Attempt recovery
sqlite3 /var/lib/gitsvnsync/gitsvnsync.db ".recover" | sqlite3 /var/lib/gitsvnsync/gitsvnsync-recovered.db

# Replace and restart
mv /var/lib/gitsvnsync/gitsvnsync.db /var/lib/gitsvnsync/gitsvnsync.db.corrupt
mv /var/lib/gitsvnsync/gitsvnsync-recovered.db /var/lib/gitsvnsync/gitsvnsync.db
sudo systemctl start gitsvnsync
```

## Getting Help

- Check logs: `journalctl -u gitsvnsync -f` or `docker logs gitsvnsync`
- CLI diagnostics: `gitsvnsync status` and `gitsvnsync validate --config /etc/gitsvnsync/config.toml`
- File an issue: https://github.com/chriscase/GitSvnSync/issues
