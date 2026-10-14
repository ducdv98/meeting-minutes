# 02: Add the /pricing/ page with Free and Pro plans

**What to build:** A visitor can open `/pricing/` from the desktop navigation, mobile menu, footer and home page and understand what Meeting Minutes costs today and what is planned, without being shown any feature, signup or price as live before it exists. The page follows the existing visual identity and responsive layout. See `.scratch/pricing-website/spec.md`, `CONTEXT.md` (Access section) and ADRs 0013 and 0014.

Content decisions:
- Two plans only: **Free plan** and **Pro plan**. An Account is an optional Free-plan state, not a third plan.
- Free: $0 forever for local Processing and the user's own provider keys, with no Account and no app-imposed usage limit (available now). A limited monthly Managed transcription allowance is planned for Accounts admitted in cohorts; no allowance number is shown.
- Pro: "Coming soon", with a larger Managed transcription allowance; no price, allowance number, payment flow or checkout.
- Explains the allowance in Processed audio minutes: each Source counts separately (a 60-minute Meeting with Microphone and System audio can use up to 120 minutes); resets monthly without rollover; when it runs out the app falls back to the user's own keys, then local Whisper.
- Signing in alone will not enable Managed transcription; the user chooses to enable it separately.
- No signup or waitlist form and no link to a nonexistent flow. No claim that cloud transcription is faster or more accurate than local Processing. No "open source" claim.

**Blocked by:** None (can start immediately). Touches the same header/nav as 01; expect a small merge conflict if run in parallel.

**Status:** done

- [ ] `/pricing/` builds and is linked from desktop navigation, mobile navigation, footer and the home page (the "Is it free?" FAQ points to it)
- [ ] Free (available now), optional Account-based Managed access (planned) and Pro (coming soon) have distinct, text-labelled availability states
- [ ] No price, allowance number, checkout, signup or waitlist form appears; no link points to a nonexistent flow
- [ ] Processed audio minutes, per-Source counting, monthly reset without rollover, fallback order, and "signing in alone does not enable Managed transcription" are explained
- [ ] No speed/accuracy comparison with local Processing and no "open source" wording
- [ ] Page is readable at the existing supported widths (down to 390px) and in light and dark themes
- [ ] Generated `llms.txt` lists the Pricing URL
- [ ] Tests cover the nav links, absence of price/checkout/signup, and the planned / coming soon labels
