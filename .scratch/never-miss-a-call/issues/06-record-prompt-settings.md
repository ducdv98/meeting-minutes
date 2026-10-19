# 06: Record prompt settings: Never for this app and the off switch

**What to build:** The prompt window gains **Never for this app**, which silences that app. Settings' Recording section gets a Record prompt switch (on by default) and the list of silenced apps by display name, each with Remove. The decider never prompts for a silenced app or when the switch is off. Settings files written before this ticket load with the switch on and an empty list. See `.scratch/never-miss-a-call/spec.md`.

**Blocked by:** 05

Status: ready-for-agent

- [ ] Core decider tests: silenced apps and the off switch never prompt; removing an app from the list prompts for it again
- [ ] Older settings files load with the defaults
- [ ] The prompt window's Never for this app adds the app to the list (UI test)
- [ ] Settings shows the switch and the silenced apps with Remove (SettingsView test)
