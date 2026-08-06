## REMOVED Requirements

### Requirement: Observation cannot block or alter input

**Reason**: The system no longer observes keyboard input at all, so there is no observation
mechanism whose capabilities need constraining. The prohibition this requirement rested on —
no `SetWindowsHookEx`, and no abstraction that would make one easy to add — is not dropped: it
is carried forward and widened by [ADR-0011](../../../../docs/adr/0011-no-keyboard-observation.md),
where it also constrains the user interface, because a chooser that avoided taking focus would
need exactly such a hook to read the keyboard.

**Migration**: None for users. Auditors previously verified this by confirming Raw Input was
the only observation path; they now verify the stronger property that no observation path
exists.

### Requirement: The buffer is bounded and memory-resident

**Reason**: Nothing fills a buffer. Bounding, page-locking and zeroizing a store that is never
written is not a weaker guarantee than before — it is an absent one, which is stronger.

**Migration**: None. No user-visible behavior depended on the bound.

### Requirement: Nothing derived from keystrokes reaches disk

**Reason**: No keystrokes are captured, so nothing can be derived from them. The privacy
obligation itself survives in a different and broader form — selected text and its conversions
must not be logged or persisted — and belongs to the capability that handles selections rather
than to a capture capability that no longer exists.

**Migration**: None. The replacement obligation is stricter, not looser: it covers text the
user selected, which capture never held.

### Requirement: The buffer does not fill in unsafe contexts

**Reason**: Password-field detection, the excluded-application list and the paused state exist
to make capture safe. With no capture they are not simplified but unnecessary. Detection was
also measured to be unreliable inside Chromium and Electron, so removing the thing it guarded
resolves a weakness that could not be fixed directly.

**Migration**: The `respect_password_fields` and exclusion settings are removed from
configuration. Existing config files carrying them are handled by this change's migration
policy rather than rejected.

### Requirement: The buffer is wiped at word and context boundaries

**Reason**: There is no buffer to wipe, so focus-change, delimiter, idle-timeout and
post-conversion wiping all lose their subject.

**Migration**: The `idle_timeout_secs` setting is removed.

### Requirement: The buffer records key positions, not characters

**Reason**: Retaining physical key identity rather than resolved characters was what made
historical typing reconstruction possible while limiting what was held. Nothing is retained.

**Migration**: None.
