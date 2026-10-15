# 11: Show the allowance and admission state

**What to build:** When signed in, Settings shows Admitted (minutes remaining this month and the reset date) or Waitlisted (with a note that local and own-key Processing keep working). When the allowance or the service is exhausted mid-Meeting, the Meeting view says where the rest of the audio was processed. See `.scratch/v1-launch/spec.md`.

**Blocked by:** 10, gateway 03 and 05 (`meeting-minutes-transcription-gateway/.scratch/accounts/issues/`)

Status: ready-for-agent

- [x] Admitted and Waitlisted states render from the gateway's Account status
- [x] Remaining minutes and the reset date are shown
- [x] Falling back after exhaustion is explained on the Meeting
