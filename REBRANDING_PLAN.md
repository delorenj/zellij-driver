---
created: 2026-01-03T06:10:00-05:00
status: planning
category: Rebranding
---
# Rebranding: zellij-driver → Perth

## New Name: Perth

**Origin**: Bon Iver song "Perth" from *Bon Iver, Bon Iver* (2011)
**Themes**: Memory, distance, reconstruction, sense of place
**Fit**: Perfect for cognitive context tracking and session memory

## Rationale

**Perth** the song is about memory and the passage of time - reconstructing context from fragments. This mirrors exactly what the tool does: reconstructs cognitive context from session fragments (commands, files, milestones).

**33GOD naming aesthetic**:
- iMi - worktree management
- Jelmore - session orchestration
- Holocene - dashboard (Bon Iver song)
- Flume - tree structure
- Bloodbank - event backbone

Perth fits: Short, evocative, Bon Iver-aligned, memory-focused.

## Rebranding Checklist

### Code Repository
- [ ] Rename GitHub repo: `zellij-driver` → `perth`
- [ ] Update remote URL in local clone
- [ ] Update package name in `Cargo.toml`: `name = "perth"`
- [ ] Update binary name: `znav` → `perth` (or keep `znav` as alias?)
- [ ] Update crate metadata (description, keywords)

### CLI Binary
**Decision needed**: Keep `znav` or rename to `perth`?

**Option A**: Rename binary to `perth`
```bash
perth pane task-123
perth pane log "Completed OAuth integration"
perth pane history task-123
```

**Option B**: Keep `znav` as friendly shorthand
```bash
# perth provides the znav binary
znav pane task-123
znav pane log "Completed OAuth integration"
```

**Recommendation**: **Keep `znav`** - it's established, memorable, and tab-navigation-focused. Perth is the project name, znav is the CLI.

### Documentation Files

**zellij-driver repository** (`/home/delorenj/code/zellij-driver/`):
- [x] `README.md` - Update title, description, references
- [ ] `PRD.md` - Rebrand to Perth
- [ ] `Cargo.toml` - Package name
- [ ] `LICENSE` - No change needed
- [ ] `INTENT_TRACKING_IMPLEMENTATION_PLAN.md` - Update references
- [ ] `CONVERSATION_SUMMARY_INTENT_TRACKING.md` - Update references

**33GOD documentation** (`/home/delorenj/d/Projects/33GOD/`):
- [ ] `ProjectOverview.md` - Rename zellij-driver → Perth
- [ ] `zellij-driver/` directory → `Perth/`
- [ ] `zellij-driver/PRD.md` → `Perth/PRD.md`
- [ ] `zellij-driver/README.md` → `Perth/README.md`
- [ ] `zellij-driver/Integration.md` → `Perth/Integration.md`
- [ ] `Jelmore/ZellijDriver.md` → `Jelmore/Perth.md`
- [ ] `Jelmore/JelmoreZellijIntegration.md` → `Jelmore/JelmorePerthIntegration.md`

### Redis Keyspace

**Decision**: Keep `znav:*` prefix or rebrand to `perth:*`?

**Pros of `perth:*`**:
- Clearer branding alignment
- Easier to identify in Redis
- Future-proof naming

**Cons of `perth:*`**:
- Migration required for existing users
- Need backwards compatibility layer
- Breaking change

**Recommendation**: **Migrate to `perth:*`** with migration script.

**New keyspace**:
```
perth:pane:{name} → Hash {session, tab, position, meta:*, last_intent}
perth:pane:{name}:history → List [intent entries]
perth:pane:{name}:artifacts → Hash {file paths}
```

**Migration script**:
```bash
#!/bin/bash
# Migrate znav:* keys to perth:* keys
redis-cli --scan --pattern "znav:pane:*" | while read key; do
    newkey="${key/znav:/perth:}"
    redis-cli RENAME "$key" "$newkey"
done
```

### Configuration Files

**Old**:
```toml
# ~/.config/zellij-driver/config.toml
```

**New**:
```toml
# ~/.config/perth/config.toml
```

**Migration**: Detect old config path, auto-migrate or warn user.

### Environment Variables

**Old**:
```bash
export ZELLIJ_DRIVER_REDIS_URL="redis://127.0.0.1:6379"
```

**New**:
```bash
export PERTH_REDIS_URL="redis://127.0.0.1:6379"
```

**Backwards compat**: Support both, prefer `PERTH_*`.

### Integration Points

#### Jelmore
```python
# Old
from jelmore.services import znav
await znav.create_pane(...)

# New
from jelmore.services import perth
await perth.create_pane(...)
```

#### Bloodbank Events
```python
# Event types
"perth.pane.created"
"perth.milestone.recorded"
"perth.session.resumed"
```

#### Shell Integration
```bash
# .zshrc
function perth_auto_snapshot() {
    local pane=$(znav pane current 2>/dev/null)
    if [[ -n "$pane" ]]; then
        znav pane snapshot "$pane" &>/dev/null &
    fi
}

precmd_functions+=(perth_auto_snapshot)
```

## Rebranding Execution Plan

### Phase 1: Internal Rename (Low Risk)
1. Update Cargo.toml package name
2. Update source code references and docs
3. Test binary still works
4. Commit: "Rebrand zellij-driver to Perth"

### Phase 2: GitHub Repo Rename (Medium Risk)
1. Rename repo on GitHub
2. Update local remote: `git remote set-url origin git@github.com:delorenj/perth.git`
3. Notify any external users (unlikely at this stage)

### Phase 3: Config Migration (Medium Risk)
1. Implement config file migration logic
2. Support both old and new env vars
3. Warn users about deprecation
4. Update all documentation

### Phase 4: Redis Migration (High Risk)
1. Create migration script
2. Test with backup Redis instance
3. Add migration command: `znav migrate`
4. Document migration process
5. Support dual keyspace during transition (6-month deprecation)

### Phase 5: 33GOD Docs Update (Low Risk)
1. Rename all directory references
2. Update cross-links in documentation
3. Update ProjectOverview.md
4. Update Jelmore integration docs

## Timeline

**Effort estimate**:
- Phase 1: S (2-4 hours)
- Phase 2: XS (30 minutes)
- Phase 3: M (4-8 hours)
- Phase 4: L (8-16 hours with testing)
- Phase 5: S (2-4 hours)

**Total**: ~M-L effort spread across implementation

## Backwards Compatibility Strategy

### Deprecation Timeline
- **v2.0**: Announce Perth branding, support both names
- **v2.1**: Migrate Redis keys, support dual keyspace
- **v2.2**: Deprecate old names (warnings)
- **v3.0**: Remove old names entirely

### Feature Flags
```toml
[compatibility]
support_legacy_keyspace = true  # znav:* keys
support_legacy_config = true    # ~/.config/zellij-driver/
warn_on_legacy = true
```

## Communication Plan

### Changelog Entry
```markdown
## v2.0.0 - Perth Rebranding

### Breaking Changes
- Project renamed from `zellij-driver` to `Perth`
- Redis keyspace migrated: `znav:*` → `perth:*`
- Config directory changed: `~/.config/zellij-driver/` → `~/.config/perth/`
- Environment variables: `ZELLIJ_DRIVER_*` → `PERTH_*`

### Migration Guide
Run `znav migrate` to automatically migrate your configuration and Redis data.
Legacy names supported until v3.0.0.

### Why Perth?
Perth (from Bon Iver's song) evokes memory and reconstruction - perfect for
a tool that preserves cognitive context across sessions.
```

### User Notification
For existing users (33GOD team):
- Slack announcement with migration guide
- Email notification
- In-CLI warning on next run
- Automatic migration offered

## Rollback Plan

If rebranding causes issues:
1. Revert Cargo.toml changes
2. Revert GitHub repo rename
3. Keep old Redis keys
4. Document as "Perth (formerly zellij-driver)"

## Next Steps

1. Get approval for "Perth" name
2. Execute Phase 1 (internal rename)
3. Test thoroughly
4. Execute Phases 2-5 sequentially
5. Update all 33GOD documentation
