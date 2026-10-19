# 01: Read which apps use the Microphone on Windows

**What to build:** A Microphone usage interface in core that returns the other apps using the Microphone right now, each with a stable app ID and a display name, and its Windows implementation. The Windows implementation reads the per-app Microphone consent store that Windows keeps for its privacy indicator: an app is in use while its last-used stop time is zero. It covers packaged apps (such as Teams), desktop apps (such as Zoom) and browsers (Meet in Edge or Chrome). The display name comes from the package name or the executable name. Meeting Minutes' own process is excluded. Help gets a hidden diagnostic line listing the apps using the Microphone right now, so the reading can be checked on a real PC before the Record prompt is built on it. See `.scratch/never-miss-a-call/spec.md`.

**Blocked by:** None (can start immediately)

Status: ready-for-agent

- [ ] The interface is defined in core, with a non-Windows stub that returns nothing
- [ ] The Windows implementation lists packaged and desktop apps currently using the Microphone, with app IDs and display names
- [ ] Meeting Minutes itself is never listed
- [ ] Help shows the current list on demand, and it updates when an app starts or stops using the Microphone
- [ ] Windows build and clippy pass
