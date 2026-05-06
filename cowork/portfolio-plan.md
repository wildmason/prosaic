# Wildmason Portfolio Plan

**Prepared:** April 2026
**Goal:** Three-phase path to $500K+ annual with a 20 hrs/week hard cap at steady state, starting from a $300K/yr contracting job.
**Scope:** Three paid Tauri apps (Mortar, Helm, Crucible) + two OSS assets (Prosaic library, Prosaic Studio authoring tool) — one coherent portfolio, not five separate companies.

---

## TL;DR

The path is: **Mortar → Helm → Crucible, sequenced not parallel, with prosaic + prosaic studio shipped OSS alongside as diffuse brand/groundswell assets.** Phase 1 ends when wildmason ARR crosses ~$300K and you have 3–6 months of runway in the bank — realistically month 28–34 from today, given the contracting constraint. Phase 2 pushes to $500K within 12–18 months of cutting the contract. Phase 3 is indefinite maintenance at <20 hrs/week, which the yearly-license desktop-app model structurally supports up to roughly $900K–$1M ARR before the cap binds.

Key strategic calls this plan makes:

1. **Prosaic and Prosaic Studio both stay 100% OSS, $0 ARR.** Their commercial value is indirect — serious, well-engineered, research-grade Rust NLG work published in public puts wildmason on the map as "the people who build thoughtful tools," and that reputation funnels developer mindshare to the paid products. Direct technical integration is Crucible-only; prosaic is not a shared runtime for Mortar or Helm and there's no near-term path for that to change. Trying to commercialize prosaic standalone would open a niche-NLG support surface for little revenue and distract from the paid products.
2. **Launch Mortar first, alone, and don't touch Helm's prelaunch until Mortar is stable.** Parallel launches during a contract are the fastest path to burnout. Mortar's empty Key Gaps list plus the Postman-flight tailwind make it the highest-probability first landing.
3. **Crucible's $100K-in-24-months target is too aggressive given its current development state.** Crucible gets $30–50K by month 24, then the serious push happens in Phase 2.
4. **JetBrains-style licensing: free for solo/non-commercial, paid per-seat for organizations.** Mortar $120/yr, Helm $96/yr, Crucible $149/yr (Crucible paid-only from day one). Annual billing only, full functionality from day one for everyone, honor-system enforcement with no DRM. This is the one pricing shape that scales revenue without scaling support burden. Everything else breaks the 20-hr cap.

---

## 1. The three phases

### Phase 1: 0 → $300K ARR (the "cut the contract" milestone)

**Primary constraint:** you are working ~18–22 hrs/week on wildmason alongside the $300K contract. Treat this number as a hard ceiling, not a starting point. Protecting weekends and sleep during this phase is how you survive to Phase 2.

**Phase exit trigger:** three consecutive months of wildmason ARR above $300K AND 3–6 months of expenses liquid in the bank. Don't cut on a single-month spike.

**Realistic timeline:** 28–34 months. The 18-month figure floated in earlier conversations is not realistic with a full-time contract running in parallel.

**What happens during Phase 1:**

- Months 0–6: finalize Mortar, beta, launch prep, content and community groundwork
- Months 6–8: Mortar public launch
- Months 8–18: Mortar customer acquisition, iteration, support infrastructure buildout
- Months 14–18: start Helm prelaunch work in background (beta, content, channel prep) — only if Mortar is stable and support burden is predictable
- Months 18–24: Helm launch, continue Mortar optimization
- Months 24–32: push both products to combined $300K ARR, start Crucible prelaunch work
- Cut the contract when trigger conditions met

### Phase 2: $300K → $500K ARR

**Primary constraint:** now fully self-employed. You can work 40+ hrs/week without destroying yourself, but only for a defined window — this is the sprint, not the new normal.

**Duration:** 12–18 months after the contract is cut.

**What happens during Phase 2:**

- Push Crucible from prelaunch to public launch (it gets the full-time treatment it needed all along)
- Scale Mortar and Helm from early-stage-launched into mature renewal-bearing businesses
- Optional: introduce a portfolio bundle ("Wildmason Suite" all three desktops at 25–30% discount vs. individual) once Crucible is shipped
- Build the support automation (community forum, in-app help, docs, FAQ) that Phase 3 depends on

### Phase 3: $500K+ ARR, 20 hrs/week hard cap

**Primary constraint:** 20 hrs/week, no exceptions, for life.

This is the phase the whole plan exists to reach. Here's what it looks like at $500K–$700K ARR, broken down to prove the cap is achievable:

| Activity | Hours/week |
|---|---|
| Customer support (email, forum moderation, license issues) | 6–9 |
| Ongoing dev and maintenance across 3 products (Tauri updates, bug fixes, small features) | 6–8 |
| Marketing / content / community (blog posts, release notes, social) | 2–3 |
| Admin (accounting, invoicing, tax, legal) | 1–2 |
| **Total** | **15–22** |

This works *only if* Phase 2 invested in the infrastructure (docs, community, self-serve) that makes Phase 3 possible. Rolling into Phase 3 with bad docs and a personal-response-to-every-email support model will put you at 35+ hrs/week regardless of revenue.

---

## 2. Sequencing and why Mortar-first is the right call

**Mortar first, because:**

- The Key Gaps section in Mortar's context is empty. It's the most launch-ready product in the portfolio.
- Postman's 2023–2024 cloud pivot created an active market of "angry defecting Postman users" — the best possible launch tailwind. Bruno has been the primary beneficiary but the market is still churning.
- Mortar's concrete competitive wedges (API governance + OpenAPI linting free vs. Postman's $49/user/mo, declarative JSON Schema assertions, 5-scope variable resolution inspector, security posture) are demonstrable in 60-second demo videos. That's what Product Hunt / HN / YouTube launches need.
- API clients are cross-language tools. The audience is every web developer, every backend developer, every QA engineer. That's the widest possible top-of-funnel in the portfolio.
- Least network-effect lock-in among the three. Git GUIs and review tools have team-standardization effects; API clients are per-developer choices.

**Helm second, because:**

- It benefits from Mortar's audience (devs using Mortar are a high-probability Helm customer).
- GitButler is a real competitive threat and having Mortar revenue in the bank gives you resilience if Helm's launch is softer than hoped.
- You need time to watch GitButler's trajectory anyway — they're the same architectural choice (Tauri/Rust) with GitHub-founder pedigree and are the one competitor who could genuinely disrupt Helm's positioning.

**Crucible third and slowly, because:**

- Its Key Gaps list is long and structurally important: rule authoring UX is raw-YAML-first, GitHub PR integration is unimplemented, inline rule customization doesn't exist, team rule sharing is unsolved, onboarding doesn't exist, seven languages out of many that matter. This is a roadmap, not a polish pass.
- Category creation is slower than category participation. Mortar and Helm have established buyer behaviors to tap into; Crucible has to teach developers that "pre-push local review" is a thing worth paying for.
- Crucible is the only product that *consumes* prosaic technically — prosaic generates Crucible's human-readable rule findings and review output. That's a real differentiator versus LLM-backed review tools that hallucinate. The differentiation narrative ("our review text is deterministic, auditable, and generated by an NLG engine designed for faithfulness, not an LLM trained on the internet") only lands once prosaic has accumulated enough OSS mindshare that the claim is verifiable rather than hand-waved. Ship prosaic publicly well before Crucible launches.

**Do not parallelize launches.** Running two launches inside 6 months of each other while contracting is how you end up hospitalized or rage-quitting. The portfolio compounds sequentially; it does not compound faster if you rush the first launches.

---

## 3. Product-by-product plan

### 3.1 Mortar

**Target customer:** solo developers (free) and developers inside for-profit organizations of any size (paid). The buyer is the org; the user is the developer.

**Pricing:**

| Use case | Price | Notes |
|---|---|---|
| Solo / non-commercial | **Free** | Full product, all features, no nag screens, no watermarks, no feature gating. Community support only. |
| Commercial (any for-profit org) | **$120 / seat / year** | Marketed as "$10/month billed annually." Actual billing is annual only. Full product, email support with 48-hr SLA. |

No team tier, no volume discount, no enterprise tier. A 5-person startup renews 5 × $120 = $600/yr. A 50-person shop renews 50 × $120 = $6,000/yr. Flat per-seat is simpler to support, easier to price, and removes the "let me talk to sales about a discount" conversation entirely.

No lifetime license option. Renewal-based revenue is a load-bearing piece of Phase 3's sustainability; one-time lifetime buys trade that away for short-term cash you don't need.

**Revenue targets (per-seat, organizations as buyers):**

- 18 months post-launch: $60–120K ARR (500–1,000 seats across ~100–200 paying orgs)
- 36 months post-launch: $150–250K ARR (1,250–2,100 seats across ~250–420 paying orgs)
- Average paying org: 5–20 seats
- Support burden at $200K (~400 paying orgs, ~1,700 seats): ~4–6 hrs/week if docs and FAQ are good. Support-per-dollar is meaningfully lower than individual-buyer models because each support contact covers multiple seats.

**Launch strategy:**

- Polish product, fix any last Key Gaps, run closed beta (50 users, 4 weeks)
- Launch week: Product Hunt + HN Show + /r/programming + /r/webdev + dev.to writeup + 2–3 YouTube videos demonstrating concrete differences vs Postman/Bruno
- Content cadence: one in-depth technical blog post per month for first 12 months (API governance deep dives, security posture, comparison posts, TOML-collections-in-git workflows)
- Community: a Discord is cheaper than a forum, faster than email, and scales better than either. Set it up day one.
- Affiliate / referral: 20% commission on first-year license for referrals. This is the cheapest marketing channel that exists; dev-tool referral programs work well.

**What to avoid:**

- Don't build an enterprise tier. The moment you do, enterprise buyers will ask for SSO, SCIM, audit logs, SOC 2, and dedicated support. Every hour spent on enterprise sales is an hour not spent on the self-serve honor-system business. If a prospect says "we can't buy without SSO," they are not your customer.
- Don't host anything. The "local-first, no cloud" promise is worth more than any hosted-sync feature.
- Don't add telemetry "just to understand users." No cloud, no account, no telemetry is part of the positioning — it is the reason buyers who hate Postman pick you.

### 3.2 Helm

**Target customer:** solo developers dissatisfied with GitKraken's subscription pricing, GitHub Desktop's feature poverty, or Tower's Electron footprint (free). Developers inside for-profit orgs looking for a power-user Git GUI that doesn't require cloud accounts (paid).

**Pricing:**

| Use case | Price | Notes |
|---|---|---|
| Solo / non-commercial | **Free** | Full product, all features, no feature gating. |
| Commercial (any for-profit org) | **$96 / seat / year** | Marketed as "$8/month billed annually." Actual billing is annual only. |

No team tier, no volume discount, no lifetime option.

The $96 is a deliberate choice — meaningfully below GitKraken ($60–96/user/yr Pro; $288/user/yr Team), in the neighborhood of Tower Personal ($69/yr) but positioned for the org buyer rather than the individual. Helm's feature depth (range-diff viewer, journaled undo, bisect UI, stacked branches + traditional workflow, image diff slider, 3-provider forge integration) justifies the price against paid alternatives while the free tier handles the "I just want a Git GUI for my side project" crowd that would otherwise be price-shopping.

**Revenue targets (per-seat, organizations as buyers):**

- 18 months post-launch: $40–80K ARR (400–850 seats across ~80–170 paying orgs)
- 36 months post-launch: $120–220K ARR (1,250–2,300 seats across ~250–460 paying orgs)

**Key competitive risk: GitButler.** Same Tauri/Rust stack, GitHub co-founder pedigree, well-funded, aggressive OSS strategy. Watch their pricing moves carefully. If they stay free-forever, you need a concrete feature moat that free can't replicate (Helm has several; range-diff viewer, journaled undo, bisect UI, hooks management UI, built-in terminal all qualify). If they introduce a paid tier at sub-$89, re-price immediately.

**Launch strategy:**

- Launch 12–18 months after Mortar is stable. Not a day sooner.
- Mortar customer email list → Helm launch announcement is probably worth 100–300 early customers on day one
- Same launch surfaces as Mortar (PH, HN, Reddit, YouTube) but with comparison-heavy content: side-by-side with GitKraken, Tower, Fork, GitButler
- Free tier of Helm for open-source contributors with 10+ merged PRs in the last year on verified projects. This is cheap marketing and fills reputational gaps with the OSS community.

### 3.3 Crucible

**Target customer:** developers inside for-profit organizations — teams that want to catch issues before PR review happens. The category is inherently collaborative; there's no meaningful solo-hobbyist use case to carve out a free tier around.

**Pricing:**

| Use case | Price | Notes |
|---|---|---|
| All commercial use | **$149 / seat / year** | Paid from day one. No free tier. No team discount. No lifetime option. |

Crucible is priced higher than Mortar and Helm because (a) the category is new and undiscounted, (b) the value is closer to "replaces a portion of SonarQube/DeepSource" which are $10–60/user/mo ($120–720/yr), and (c) the audience is narrower (power users and small teams) so per-seat volume will be lower. Even at $149, Crucible remains dramatically cheaper than the enterprise-SaaS category it competes with.

**Why paid-only for Crucible specifically:**

- Pre-push code review creates value when a team agrees on shared rules. A solo developer reviewing their own code pre-push has marginal benefit; a 5-person team enforcing rules consistently has compounding benefit. The free-for-solo model that fits Mortar and Helm doesn't fit here.
- No free tier = no "is my use commercial?" ambiguity for Crucible. Simplifies licensing language.
- Preserves the option to offer a limited free trial (e.g., 30 days, full functionality) for evaluation without standing up a permanent free tier.

**Revenue targets (per-seat, organizations as buyers):**

- 24 months from today (roughly 6–12 months post-launch given the sequencing): $30–50K ARR (200–335 seats across ~40–70 paying orgs)
- 36 months from today: $70–130K ARR (470–870 seats across ~95–175 paying orgs)
- 48 months from today: $150–280K ARR (once category creation takes hold)

**What has to be true before Crucible launches:**

- Rule authoring UX is no longer raw-YAML-first. At minimum: rule templates, in-app pattern playground, inline rule customization from the issue card.
- Onboarding flow exists. Crucible's value is invisible if the first-run experience doesn't demonstrate the semantic diff + blast radius + rule findings in under two minutes.
- GitHub PR review integration is shipped. Even if local modes are the differentiation, PR integration is table-stakes for many buyers.
- Three case studies / demo videos showing real-world use on recognizable open-source codebases.

The current Key Gaps list estimates 6–9 months of focused work to close, which is why Crucible is third in the sequence.

### 3.4 Prosaic and Prosaic Studio

**Commercial strategy: both stay full OSS, $0 revenue target, diffuse groundswell assets for the paid products.**

**What prosaic actually is — and isn't.** Prosaic is a deterministic, rule-based, template-based, static-inference NLG library. It does not use AI, LLMs, or any learned model. It is research-grade (REG via Dale & Reiter, Centering Theory transitions, RST-labeled discourse markers, faithfulness scoring) and built in Rust. Its output is reproducible, auditable, and by construction cannot hallucinate — not because it's "AI done right," but because it's a fundamentally different paradigm from generative AI. The positioning is **"when you need generated text that's controllable, auditable, and impossible to hallucinate, you use prosaic. When an LLM is the right tool, you use an LLM."** Do not conflate the two in any marketing copy.

**Direct technical integration in the portfolio: Crucible only.** Prosaic drives Crucible's human-readable rule-violation messages, review summaries, and PR descriptions — exactly the surface where an LLM would be tempting but hallucination is unacceptable (wrong explanations of real code findings are worse than no explanations at all). Mortar and Helm do not use prosaic today and there is no clear near-term path for them to do so; their natural-language needs are small (tooltips, error messages, docs) and don't justify an NLG library. Don't force prosaic integration into Mortar or Helm for narrative coherence — it would be make-work with no product value.

**Portfolio value of prosaic is therefore indirect.** The argument is:

- Publishing a serious piece of research-grade Rust NLG engineering as OSS establishes wildmason as "a brand that builds thoughtful, well-engineered developer tools." That reputation is diffuse but real — it's the same reason Rust developers trust tokio, why Go developers trust cockroachdb's OSS work, why a particular indie studio commands attention even in unrelated product categories.
- Some fraction of prosaic's OSS users, stargazers, or blog-post readers discover wildmason → look at the paid products → become Mortar/Helm/Crucible customers.
- For Crucible specifically, the prosaic integration is a concrete, verifiable technical differentiator versus LLM-backed competitors.

This is a slower, more indirect funnel than "prosaic is the engine under all our AI features" would have been, but it's honest, and it avoids the credibility risk of overstating the connection.

**Prosaic Studio.** A dedicated IDE for prosaic authoring, still in development. Not a playground or casual try-it tool — the end goal is a serious authoring environment: grammar/template/vocabulary editors with project-level structure, output preview, testing harness, debugging, version control integration, maybe a grammar-validation pass. Matt is comfortable shipping Studio 100% OSS, on the same groundswell logic as prosaic itself.

**Distribution: Tauri desktop app + WASM web build.** Same codebase, two delivery modes:

- **Tauri desktop app** is the primary experience — full IDE, local file system access, persistent projects, full performance. This is the install-and-use-for-real version.
- **WASM web build** runs Studio in-browser, likely as a trimmed variant (no file system outside in-memory / IndexedDB, limited project size). This is the zero-install top-of-funnel: embed it on the prosaic website, link it from blog posts, share it on social media. Someone sees a prosaic blog post → clicks "try it" → is authoring grammars in a browser tab within 5 seconds. That's an unusually powerful discovery mechanic for a developer tool.

This dual-mode distribution is genuinely strategic:

- It lowers the barrier to entry for non-Rust developers. A Python/Node/Ruby developer who needs deterministic NLG can author grammars in the browser, then install the desktop app once they're committed.
- It gives prosaic a live, interactive demo surface. "Here, try it right now in your browser" is a much better top-of-funnel asset than a Rust crate docs page or even a video.
- It creates two SEO/community surfaces — the web build for discovery, the desktop build for the committed authoring audience.
- Studio-as-a-Tauri-app doubles as a public demonstration of the wildmason stack — a walking advertisement for "the kind of polished desktop apps wildmason builds." That reputation transfers to the paid Tauri products.
- The web build can also be embedded inside prosaic's own documentation pages, turning the docs into an interactive experience rather than static reference.

Because Studio is an IDE and not a playground, its build-out is non-trivial. Don't treat it as "small companion tool" effort. Ship it when it's genuinely IDE-grade — a half-baked IDE would damage the prosaic story more than having no Studio at all. The WASM build can launch alongside the desktop build or even lag it; don't block a desktop release on the web build being feature-complete.

**What to do with prosaic + studio:**

- Keep both MIT/Apache dual-licensed, fully open source. Never close any of it.
- Publish a technical blog post series on prosaic's innovations (REG via Dale & Reiter, Centering Theory transitions, faithfulness scoring, RST-labeled discourse markers). Each post is SEO bait for the developer audience.
- Submit to NLG research venues (INLG, ACL) if the academic angle appeals. Not strictly necessary but it builds research-grade credibility in a way that marketing copy can't.
- Ship prosaic publicly 6–12 months before Crucible launches so the "prosaic-backed review" claim is verifiable when Crucible arrives.
- Ship prosaic studio as a second OSS asset — ideally with an interactive web demo (prosaic compiles to WASM already) so people can try it without installing anything.
- Write a Show-HN / Product-Hunt-style launch for each. "Deterministic NLG library written in Rust" and "visual authoring tool for deterministic NLG" are two independent launch surfaces.
- Zero support target. "Prosaic and Studio are MIT; use at your own risk; PRs welcome." If either becomes a support burden, narrow the feature surface, not your time budget.

**When to reconsider commercializing prosaic (or Studio) separately:**

Only if either develops commercial pull you didn't create on purpose — e.g., a pharma company, regulatory tech firm, or technical documentation vendor asks about commercial licensing, sponsored feature work, or bespoke integration. Those are inbound-driven, negotiated as custom deals, by definition low-volume/high-ticket, and structurally at odds with the 20-hr cap at scale. If they appear, handle them as one-off consulting engagements (fixed-price, time-boxed) rather than standing up a commercial product surface. Don't proactively build a commercial product motion for prosaic or Studio in Phase 1 or Phase 2.

---

## 4. Portfolio pricing math

At steady state (Phase 3), the portfolio revenue math needs to work at the 20 hrs/week cap. With per-seat pricing and organizations as the buyer, the model looks like this:

| Scenario | Mortar ($120/seat) | Helm ($96/seat) | Crucible ($149/seat) | Paying orgs | Total ARR | Est. support/week |
|---|---|---|---|---|---|---|
| Phase 1 exit ($300K) | 1,400 seats = $168K | 800 seats = $77K | 400 seats = $60K | 200–400 | **~$305K** | 5–7 hrs |
| Phase 3 target (~$600K) | 2,600 seats = $312K | 1,800 seats = $173K | 900 seats = $134K | 400–800 | **~$619K** | 10–13 hrs |
| Stretch ($1M) | 4,500 seats = $540K | 3,000 seats = $288K | 1,700 seats = $253K | 700–1,400 | **~$1.08M** | 15–18 hrs |

Average seats per paying org: 5–20. A 5-person startup renews once/year for ~$600 combined ($120 × 5 if Mortar only, up to $1,825 if they buy all three products for 5 seats each). A 50-person shop is ~$6K–$18K.

**Why this is a better fit for the 20-hr cap than individual-buyer pricing:**

- **Support scales with the number of paying entities, not the number of seats.** 400 paying orgs means 400 buyers you're on the hook for — not 2,000 individuals. Each org typically has one procurement contact, one technical lead, and a handful of IC users who escalate through them.
- **Renewals are one conversation per org per year.** 400 renewal conversations is a manageable volume; 6,000 individual renewals is not.
- **Annual billing compresses payment events.** One invoice per org per year instead of monthly. Failed-payment recovery, refund processing, plan changes all scale with org count, not seat count.
- **The free-solo tier absorbs the "will I pay?" indecision of individual developers.** Those users get full functionality forever and cost you nothing beyond community-forum traffic.

Support burden estimates assume reasonable investment in self-serve docs, community forum, in-app help, and automated onboarding. Without that investment, multiply by 2–3x.

The Phase 3 target (~$619K, ~10–13 hrs/week support, ~600 paying orgs) is the specific plan. The stretch case ($1.08M at ~1,000 orgs) shows there's meaningful headroom above $500K before the 20-hr cap binds. Beyond ~1,500 paying orgs the support math starts to strain; that's a Phase 4 "do I hire a part-time support contractor" problem and is deliberately out of scope for this plan.

---

## 5. Licensing, enforcement, and checkout

**Licensing posture: JetBrains-style honor system.**

A license key is required to unlock the paid/commercial mode of Mortar and Helm (and to unlock Crucible at all). But there is no DRM, no phone-home enforcement, no aggressive anti-piracy, no online activation. The product works fully offline. A pirated key still works. This is deliberate.

The principle: **legitimate businesses treat other legitimate businesses as such.** Any for-profit organization willing to sign a commercial agreement for an IDE (JetBrains), a design tool (Figma), or a communication tool (Slack) will sign one for a developer tool. Orgs that pirate are self-selecting out of the paid market regardless — spending engineering time to block them is spending time against the 20-hr cap for zero revenue gain.

**What the license key gates:**

- Mortar: switches the "Commercial use" attestation from required to unlocked. Nothing else changes.
- Helm: same.
- Crucible: gates the entire application. No license, no product.

**What the license key does NOT do:**

- Phone home.
- Transmit any telemetry.
- Call out to a server on launch.
- Expire if offline for N days.
- Restrict feature availability.
- Watermark output.

Validation is cryptographic and local (signed license payload, public key bundled with the binary).

**Commercial-use definition (for Mortar and Helm licensing language):**

*"A commercial license is required when this software is used by or on behalf of a for-profit organization, or in the course of generating revenue. Use by individuals for personal projects, educational purposes, open-source contributions, or within registered non-profit organizations is free."*

This language is clean enough for a lawyer to tighten later without changing the substance. Edge cases (consultants billing hourly using the tool to deliver work for a paying client, employees of a for-profit who also use it for personal projects outside work) resolve the same way JetBrains resolves them: if you're getting paid to use it, someone needs a license.

**Checkout requirements:**

Honor-system licensing only works if the buy path is frictionless. The moment procurement gets involved, the "just expense it" individual-developer purchase motion breaks down.

- **Self-serve checkout.** Credit card, company card, or net-30 invoice. No sales rep. No quote-to-cash cycle.
- **5-minute purchase flow.** Land on pricing → enter email + card → receive license key via email within 60 seconds.
- **Stripe Checkout is the right tool for Phase 1.** Stripe Billing for subscriptions, Stripe Tax for VAT/sales-tax compliance (do not skip this — indie-SaaS founders who skip Stripe Tax end up doing VAT math manually, which is exactly the kind of admin that breaks the 20-hr cap), Customer Portal for self-serve seat management and invoice downloads. Paddle is the merchant-of-record alternative if Stripe Tax's complexity ever becomes too much; Polar is a lighter-weight option if Stripe pricing ever bites.
- **License delivery, renewal reminders, failed-payment dunning, and refund processing must all be automated.** If you're manually issuing replacement license keys, you're already off-plan.
- **Volume discounts: none.** Flat per-seat. This removes the "let me talk to sales" conversation entirely, which is the single biggest threat to self-serve purchase motion at the 20-hr cap.

**What to do when an enterprise buyer asks for SSO / SOC 2 / legal redlines / procurement review:**

Politely decline. They are not your customer. This will feel wrong in the moment — a 200-seat enterprise deal at $120/seat is $24K of ARR. But that deal comes with a 3–6 month procurement cycle, a legal negotiation, a security questionnaire, SSO implementation, SOC 2 audit, and a dedicated point of contact for the life of the account. That's a full-time job, not a transaction. The honor-system model is only sustainable if you don't chip away at it one "just this once" enterprise deal at a time.

---

## 6. Support model — the one thing that makes or breaks Phase 3

Everything about Phase 3 depends on support burden staying below ~15 hrs/week at $500K+ ARR. That is only possible with deliberate upfront investment in both infrastructure and tier discipline.

**Free users get community support only.** No email support. No direct support channel. The channels available to free users are:

- Discord community (peer support, occasionally moderator intervention)
- GitHub Issues (for reproducible bugs, with a required template)
- Docs, FAQ, in-app help

This is non-negotiable. Free users are 60–90% of your user base by count but 0% of your revenue. Providing them email support at scale is the fastest way to destroy the support economics of the paid business. Be warm and generous in the community spaces; be strict about not routing free users to email.

**Paid commercial users get email support with a 48-hour SLA.** Service Level Agreement — a published commitment to respond within a time window (48 hours in this case, business days). "Respond" means acknowledge and triage, not necessarily resolve. Tickets are worked during scheduled support windows (2×/week), not always-on.

If a paid user asks for faster-than-48hr response, the answer is "we don't offer that tier." Not "we can look into it." The moment you take one VIP support contract, you've created the obligation to maintain it, and you're on the path to synchronous-support hell.

**What enables the SLA without breaking the cap:**

- **Docs that actually answer questions.** Not API reference dumps — task-oriented walkthroughs with screenshots. Write these during Phase 1 and update them continuously. Every time you answer the same question twice over email, it goes into docs.
- **In-app help.** Tooltips, contextual help panels, an embedded docs viewer. For Mortar, every panel should answer "what does this do and when would I use it?" inline. For Helm, same for every menu item. For Crucible, the rule editor particularly needs this.
- **Community forum, ideally Discord.** Let users help users. Seed it with 50–100 invited beta users before public launch. Moderate 2–3 times per day during Phase 1 ramp, drop to once-per-day in Phase 2, maybe every other day in Phase 3.
- **FAQ that pre-answers emails.** Different from docs — specifically "things people email me about." When you notice a pattern in support email, add an FAQ entry and update the auto-reply.
- **Structured bug template.** Require reproduction steps, environment info, and logs. Bugs without these get a polite template asking for them. This cuts support time roughly in half.
- **Scheduled "support hours" instead of always-on email.** Twice a week (e.g., Tue/Thu mornings, 2 hrs each) for triaging inbound. Outside those windows, your support email is unread. Phase 1, you can stretch to 3×/week; Phase 3, twice is the cap.
- **License automation.** License generation, delivery, renewal, seat changes, and refund should all be automated via Stripe + your license-signing service. Manually issuing a replacement license is 10 minutes; if you do it 3×/week, that's 30 min/week you can't afford at 20 hrs.

This is the Phase 1 homework that enables Phase 3. Do it while Mortar is ramping. If you skip it you will hit Phase 3 at 30+ hrs/week and hate your life.

---

## 7. Risks and dependencies

**Bruno's trajectory determines Mortar's ceiling.** If Bruno's Gold tier starts capturing the paying segment, Mortar's differentiation needs to be sharper. Watch Bruno's release cadence and pricing pages quarterly.

**GitButler's trajectory determines Helm's ceiling.** Same monitoring cadence. The specific risk is GitButler introducing a compelling paid tier that Helm has to undercut or match.

**Tauri 2 ecosystem health.** You're deeply committed to this stack. If Tauri 2 stagnates, gets absorbed, or has a security issue that erodes trust, you inherit all of that. Subscribe to their releases, watch their GitHub issues, maintain a relationship with the project if possible.

**The "renewal question."** Yearly licenses have renewal risk that one-time purchases don't. First full renewal cycle is 12 months post-launch; renewal rate will determine Phase 2 sustainability more than any other variable. Plan to obsess over renewal UX in the 30-day window before renewal: in-app prompts, email sequences, what's-new-since-you-bought summaries.

**Macro/recession risk.** Yearly subscriptions are more resilient than monthly but still exposed during budget cuts. If a recession hits in Phase 1 or 2, expect a 10–25% renewal/acquisition hit. The honor-system per-seat model is more resilient than tiered SaaS here — there's no cheaper tier for buyers to downgrade to, so the decision is binary (renew or cancel), and the low absolute price per seat ($96–$149) keeps the cancellation threshold high.

**Honor-system compliance erosion.** The model assumes orgs self-attest commercial use. Over time, some orgs will use Mortar/Helm in commercial settings without paying. This is expected and built into the plan — don't chase enforcement, don't add telemetry "just to measure the gap." The correct response is to keep improving the product and assume the paying share of the market is roughly constant as the total user base grows. If compliance drops noticeably (e.g., obvious corporate IP ranges in the Discord with no matching paid accounts), address it through friendly targeted outreach, not technical enforcement.

**Your own health.** The biggest risk is you burning out in month 18 of a contract-plus-two-products schedule. Budget aggressive WLB protection during Phase 1 — it is the single thing that could take the whole plan off the rails.

---

## 8. Decision triggers

Write these down now so future-you isn't relitigating them under stress.

**When to start Helm prelaunch work:** after Mortar has hit $60K ARR AND support burden has stabilized at <4 hrs/week for 60 consecutive days. If support is still spiky at month 18, delay Helm.

**When to launch Helm:** after Mortar has hit $100K ARR AND the Helm-specific Key Gaps (conflict editor interaction, Azure DevOps, onboarding) are closed or consciously deferred. Don't launch Helm into a market you're still scrambling to support.

**When to start Crucible's launch push:** after combined Mortar+Helm ARR hits $200K AND Crucible's blocking Key Gaps are closed (rule authoring UX, GitHub PR integration, onboarding).

**When to cut the contract:** wildmason ARR above $300K for three consecutive months AND 3–6 months of expenses liquid in the bank. Not on a single spike.

**When to introduce the portfolio bundle:** after Crucible has been launched for 6 months. Not before — you need real data on Crucible's standalone conversion first. Structure the bundle as a flat per-seat price covering all three products at ~25–30% discount vs. buying each separately.

**When to consider a fifth product:** not until all three existing products are past $150K ARR each or you're in Phase 3 with available bandwidth. Adding a fourth simultaneously is how this plan fails.

**When to hire:** probably never. Employees break the 20-hr cap structurally — hiring obligations consume more than they free. If specific tasks genuinely need to be delegated (security reviews, legal, accounting), contract them out to specialists. This is a solo business by design.

---

## 9. 18-month action list

Month-by-month is too granular to commit to, but here's the 18-month "if I'm not doing this, I'm off-plan" list:

Months 0–3: finish Mortar, ship beta to 50 private users, write initial docs, set up Discord, stand up Stripe Checkout + Stripe Billing + Stripe Tax, build the license-signing service, draft 4 blog posts on differentiation wedges. Finalize the commercial-use licensing language (legal review). Publish pricing publicly at "$10/month billed annually."

Months 3–6: Mortar 1.0 launch week (PH, HN, Reddit, dev.to, YouTube). Triage the first month of support and feature requests. Batch bug fixes into a 1.1 release.

Months 6–12: Mortar content cadence (monthly technical posts, weekly Discord presence, quarterly deep-dive video). Grow the email list to 3,000+. Hit $40–60K ARR by month 12.

Months 12–18: Mortar optimization + Helm prelaunch prep. Beta Helm to Mortar's most engaged customers. Start drafting Helm launch content. Hit $80–120K combined ARR by month 18.

If you're on this trajectory at month 18, Phase 1 is on track. If you're at <$50K ARR by month 18, something in the launch or the market read is wrong and the whole plan needs a re-read before adding Helm to the pile.

---

## 10. What I got wrong in the prior competitive-analysis doc

For completeness: the `/cowork/competitive-analysis.md` written earlier in this session was pitched at "commercialize prosaic as enterprise SaaS." That analysis is wrong for your goals — SOC 2, enterprise sales cycles, and compliance certifications all structurally break the 20-hr cap at $500K+. The competitive-analysis doc remains useful as a reference on the NLG market and what competitors look like, but the commercialization recommendations in it (Team tier at $500/mo, Business tier at $2.5K/mo, Enterprise at $75K+) are superseded by this document.

The correct commercialization posture for prosaic (and Prosaic Studio) is stated in §3.4 above: pure OSS, $0 revenue, diffuse groundswell asset for the paid products. Also note: prosaic is NOT an AI engine. It's deterministic rule-based NLG. The earlier competitive-analysis doc accidentally framed it in AI-adjacent terms in places — treat that framing as superseded.

---

## One-page summary

| | |
|---|---|
| **Phase 1 goal** | Replace $300K contract income from wildmason portfolio |
| **Phase 1 timeline** | 28–34 months realistic |
| **Phase 1 products** | Mortar (launch month 6–8), Helm (launch month 18–24), Crucible (prelaunch only) |
| **Phase 2 goal** | $300K → $500K+ ARR |
| **Phase 2 timeline** | 12–18 months after cutting the contract |
| **Phase 2 products** | Crucible public launch; optimize existing |
| **Phase 3 cap** | 20 hrs/week hard cap, indefinite |
| **Prosaic posture** | Full OSS, $0 ARR target. Deterministic rule-based NLG (not AI). Direct technical consumer in portfolio: Crucible only. Portfolio-wide value is diffuse brand/groundswell. |
| **Prosaic Studio posture** | Full OSS alongside prosaic. Authoring tool POC. Second launch surface for the prosaic groundswell story. |
| **Licensing model** | JetBrains-style: free for solo/non-commercial, paid per-seat for for-profit orgs. Honor system, no DRM, no telemetry. |
| **Pricing** | Mortar $120/seat/yr ("$10/mo billed annually"), Helm $96/seat/yr ("$8/mo billed annually"), Crucible $149/seat/yr. Annual billing only. |
| **Billing** | Annual only, no monthly option. Stripe Checkout + Stripe Billing + Stripe Tax. |
| **No team tier** | Flat per-seat pricing. 5-person team = 5 × price. Removes procurement conversations. |
| **No enterprise tier** | No SSO, no SCIM, no SOC 2, no dedicated support. Polite decline to enterprise asks. |
| **No lifetime licenses** | Renewal revenue is load-bearing for Phase 3; don't trade it away. |
| **Free-user support** | Community Discord + GitHub Issues + docs. No email support. |
| **Paid-user support** | Email, 48-hr SLA, scheduled 2×/week support windows. |
| **No hired employees** | Breaks the WLB model; use specialist contractors only (legal, accounting, security). |
| **Single-product focus** | One launch at a time during Phase 1. Mortar first. |
| **Success metric** | Phase 1: wildmason ARR crosses $300K. Phase 3: 20 hrs/week, ≥$500K, indefinitely. |
