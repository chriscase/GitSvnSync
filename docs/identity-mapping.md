# Identity Mapping

GitSvnSync transparently maps author identities between SVN and Git so that commits always appear under the correct developer's name.

## The Challenge

SVN and Git represent identity differently:

| System | Format | Example |
|--------|--------|---------|
| SVN | Username only | `jsmith` |
| Git | Name + Email | `John Smith <jsmith@company.com>` |

Additionally, Git distinguishes between **Author** (who wrote the code) and **Committer** (who applied it to the repository).

## How GitSvnSync Handles It

### SVN → Git

When a commit by `jsmith` is synced from SVN to Git:

```
Git Author:    John Smith <jsmith@company.com>    ← original developer
Git Committer: GitSvnSync <sync@company.com>      ← daemon (audit trail)
Git Message:   "Original commit message"
               ""
               "Synced-from: svn r1234 by GitSvnSync"
```

The mapping from `jsmith` → `John Smith <jsmith@company.com>` comes from:
1. The authors.toml mapping file (checked first)
2. LDAP/Active Directory lookup (if configured)
3. GitHub API user lookup (if configured)
4. Fallback: `jsmith <jsmith@company.com>` (using configured email domain)

### Git → SVN

When a commit by `John Smith <jsmith@company.com>` is synced from Git to SVN:

```
SVN Author:    jsmith                              ← set via svn:author property
SVN Message:   "Original commit message"
               ""
               "Synced-from: git abc1234 by GitSvnSync"
```

The reverse mapping from `John Smith <jsmith@company.com>` → `jsmith` uses the same sources in reverse.

This requires the SVN server's `pre-revprop-change` hook to allow author modification. See [getting-started.md](getting-started.md#step-2-prepare-your-svn-server).

## Mapping File Format

`/etc/gitsvnsync/authors.toml`:

```toml
[authors]
# SVN username = { Git identity }
jsmith = { name = "John Smith", email = "jsmith@company.com", github = "johnsmith" }
janedoe = { name = "Jane Doe", email = "jane.doe@company.com", github = "janedoe" }
buildbot = { name = "Build Bot", email = "buildbot@company.com" }

[defaults]
# Fallback email domain for unmapped users
email_domain = "company.com"
```

The `github` field is optional and used for matching GitHub OAuth identities.

## LDAP Integration

For large organizations, configure LDAP for automatic lookups:

```toml
[identity]
mapping_file = "/etc/gitsvnsync/authors.toml"
email_domain = "company.com"
ldap_url = "ldaps://ldap.company.com:636"
ldap_base_dn = "dc=company,dc=com"
ldap_bind_dn = "cn=gitsvnsync,ou=services,dc=company,dc=com"
ldap_bind_password_env = "GITSVNSYNC_LDAP_PASSWORD"
```

LDAP lookup queries `uid` and `mail` attributes to build the mapping.

## Lookup Priority

1. **Explicit mapping file** — fastest, always checked first
2. **LDAP/AD** — queried for unknown users, results cached
3. **GitHub API** — query user by username
4. **Fallback generation** — `username@configured-domain.com`

Lookups never fail silently. Unknown users are mapped with a warning in the log.

## Managing Mappings

### Via CLI

```bash
gitsvnsync identity list                           # Show all mappings
gitsvnsync identity add jsmith "John Smith" jsmith@company.com
gitsvnsync identity remove old_user
```

### Via Web UI

Navigate to Configuration → Identity Mapping in the web dashboard.

### Via File

Edit `/etc/gitsvnsync/authors.toml` directly. The daemon reloads it automatically on change.

## Security Considerations

- The sync daemon commits on behalf of developers using Git's Author/Committer distinction
- All synced commits include a `Synced-from:` trailer identifying the sync origin
- The daemon's Git committer identity provides a clear audit trail
- On the SVN side, the `svn:author` is set to the original developer, with a message trailer noting the sync
- LDAP bind credentials are stored as environment variables, never in config files
