# Prosaic — Commercial Market Analysis

**Prepared:** April 2026
**Scope:** Rule-based / deterministic NLG vendors only (excluding general-purpose LLM providers)
**Question on the table:** What would it take for prosaic to access commercial markets, and is that market worth the effort?

---

## TL;DR

The commercial rule-based NLG market is smaller than it was three years ago and still contracting at the commodity end, but it has consolidated into a defensible specialty: **deterministic text generation for regulated industries**. The fluency battle is lost — LLMs won it decisively and there is no commercial future in selling "NLG that produces nicer sentences." The audit-trail battle, by contrast, is still being fought, and prosaic is unusually well-positioned to compete in it.

The short answer: **yes, commercial markets are accessible**, but only if you stop competing on "natural-sounding output" and start competing on **reproducibility, faithfulness, and explainability**. The single most commercially valuable feature prosaic already has is `score_faithfulness` / `assert_faithful!` — that is not a nice-to-have, that is the product. Almost everything else in the README is table stakes.

Realistic revenue picture over 24 months: $300K–$1.2M ARR is plausible with focused go-to-market; $5M+ requires a dedicated founding team and a vertical wedge (pharma, fintech, or regulated e-commerce). Without that team, this is a $50K–$150K/year side-project business with a strong developer-tool brand.

---

## 1. What prosaic actually is, from a commercial lens

Stripping the README down to what matters for positioning:

**Technical differentiators that are commercially relevant:**

- Deterministic, reproducible output (`Variation::Fixed`, `Variation::Seeded`). No LLM in the pipeline. Same input → same output, forever.
- Faithfulness scoring built in (`score_faithfulness`, PARENT-style). This is *rare*. Most competitors bolt this on via separate QA vendors or not at all.
- Discourse-level capabilities — Centering Theory transitions, REG via Dale & Reiter, RST-labeled discourse markers, clause aggregation, gapping. This is research-grade NLG, not a templating library.
- Multi-language from day one (EN/ES/DE) with a clean `Language` trait. Adding a fourth language is a tractable engineering project.
- `no_std + alloc` compatible, WASM-ready, Rust performance profile (milliseconds, single binary).
- MIT/Apache dual license — friendly for enterprise adoption.

**Technical capabilities that are table stakes (not differentiators):**

- Templating with pipes, conditionals, pluralization, articles, conjugation. Every competitor has this.
- Synonym rotation, list-style cycling, tense/aspect composition. Nice, but not billable.
- Builder API, derive macro, streaming render. Developer experience, not market positioning.
- CLI / JSON-lines I/O. Necessary, not sufficient.

**Gaps that are commercial blockers:**

- No hosted product. No web UI. No authoring surface for non-engineers — `prosaic-project` is a CLI scaffold, not a Studio.
- No audit log / change-tracking / document versioning layer (which is the actual product pharma and finance buy).
- No compliance certifications, no SOC 2, no HIPAA BAA capability, no data residency story.
- No integrations with the systems buyers already have (SAP, Oracle, Salesforce, Tableau, Power BI, Workday, Veeva).
- No vertical domain packs beyond code / git / PR / release — i.e. beyond the engineer's own domain. Pharma vocab, financial reporting vocab, EU AI Act product-description vocab do not exist.
- English REG + Centering are mature; Spanish and German ship the primitives but haven't been validated on a real corpus. Buyers in regulated industries will demand proof.
- No published benchmarks against human-written output or against competitors.

This gap list is the 24-month commercialization roadmap, and it's more work than the library itself.

---

## 2. The market: who is actually paying for rule-based NLG in 2026

The total NLG market is widely reported at $1–2.5B in 2026 (MarketsandMarkets, Fortune Business Insights, IDC) with 15–25% CAGR projections through 2030, but those figures pool rule-based NLG with LLM-based text generation. Stripped down to the deterministic / rule-based subset, realistic addressable spend is a fraction — best estimate **$150M–$300M globally in 2026**, heavily concentrated in enterprise license deals. It is a boutique market.

### 2.1 Vendor landscape

| Vendor | Status (April 2026) | Pricing signal | Positioning |
|---|---|---|---|
| **Arria NLG** | Distressed. NZ entity narrowly avoided IRD liquidation in 2024; court-ordered to pay an ex-employee ~NZ$70K in unpaid wages (Nov 2025 Newsroom/NBR reporting). Still selling. | Enterprise custom; $100K+ ACV inferred from G2 reviews citing "prohibitive" cost | Broad enterprise; pivoting to hybrid LLM+rules |
| **Yseop** | Healthy and growing. €10M growth round from Claret Capital Partners (Sept 2025). Strategic partnership with Novartis. TIME Best Inventions 2025. | Enterprise custom; $150K+ ACV inferred | **Pharma / life sciences** — regulated clinical & regulatory writing |
| **AX Semantics / axite** | Active. Rebranded to "axite" in 2024–2025 with a value-based pricing pivot. Pricing page now shows **"Fixed Project Fee" (Catalyst) and "Custom Quote" (Professional / Enterprise+)** — the previously published $279 / $799 / $1,599 tiers appear to have been pulled. | Published tier structure with sales-led pricing | E-commerce product content, multilingual |
| **Automated Insights (Wordsmith)** | Acquired into Stats Perform. Pivoted from AP earnings → **sports analytics and betting narratives**. No longer a standalone NLG platform. | Not disclosed | Sports / betting, closed garden inside Stats Perform |
| **Narrative Science / Quill** | Acquired by Salesforce (Nov 2021). Folded into Tableau as Data Stories / Pulse narratives. **Dead as an independent product.** | N/A — feature, not product | Internal BI only |
| **Retresco / textengine.io** | Active, quiet. Added LLM-augmented generation Dec 2023. No recent funding or strategic news. | Opaque / custom | German e-commerce & sports |
| **United Robots** | Active, growing regionally. Scandinavian publisher network. | Custom; priced on coverage + data-source depth | Automated local journalism |
| **Phrasee** | Transitional. Marketing copy; increasingly LLM + brand-rail positioning rather than pure rule-based. | Custom enterprise | Email / SMS / push copy optimization |
| **RosaeNLG** | **Archived March 2026** by LF AI & Data TAC. Reason given: templates being replaced by LLMs for most use cases. | Free (was) | OSS — dead |
| **SimpleNLG** | Maintenance mode. Ehud Reiter has signaled retirement summer 2026 on his blog. Low activity. | Free | Academic OSS — zombie |
| **2txt** | No current web presence found. Presumed defunct or absorbed. | — | — |

### 2.2 The pattern to see

Three things are happening at once.

**One:** Rule-based NLG as a standalone product category is losing the commodity fight. RosaeNLG is archived. SimpleNLG is a retirement project. Narrative Science was absorbed into a BI platform. Automated Insights was absorbed into a sports-data platform. The open-source ecosystem is signaling — explicitly, in RosaeNLG's archival notice — that templates are being replaced by LLMs "for many use cases, with less configuration effort." That is not a recoverable position if you try to compete on fluency.

**Two:** The vendors that are healthy are the ones that picked a regulated vertical. Yseop's €10M round and Novartis partnership are the clearest evidence in the market that rule-based NLG is a growth business *inside the pharma documentation niche*. United Robots is sustainable on publisher relationships and local-news automation. AX Semantics is defending the mid-market e-commerce segment, partially, by leaning into EU AI Act compliance angles. The common thread is that each has found a buyer for whom "the LLM might hallucinate" is a disqualifying problem.

**Three:** The incumbent that tried to stay broad — Arria — is in visible distress. "Broad enterprise NLG" is not a commercial position that holds up in 2026.

### 2.3 Why LLMs don't (yet) take the whole market

This is the core of why commercial opportunity still exists. Rule-based NLG retains four structural advantages that LLMs have not closed and may not close:

**Reproducibility.** Financial, medical, and legal workflows require that the same input produce the same output across runs, dates, and model versions. LLMs do not guarantee this even at temperature 0 — model updates, tokenizer changes, and provider rollouts break reproducibility. This alone disqualifies LLMs from a lot of regulated workloads.

**Faithfulness by construction.** A rule-based engine that fills slots from structured data cannot hallucinate a number it wasn't given. An LLM can, and at scale will. In pharma CSRs, SEC filings, and clinical documentation this is a binary: you can't ship a system that sometimes invents data.

**Explainability.** Regulators increasingly want "why did this output say X?" A rule-based trace ("template `code.renamed` was selected at Salience::High because `consumer_count`=47; slot `old_name` filled from context key `old_name`") is auditable. An LLM trace is not, and the EU AI Act is going to make this worse for LLM vendors through 2026–2027.

**Cost and latency at scale.** For high-volume, low-variance content (millions of product descriptions, sports recaps, earnings summaries), a rule-based engine is 10–100× cheaper per item than an LLM API and runs at single-millisecond latency. This matters less as LLM inference gets cheaper but still matters for on-prem, edge, and embedded deployments.

Prosaic hits all four. Structurally it is the right product for the regulated-content buyer. The question is whether the company that ships it exists.

---

## 3. Prosaic vs. competitors — feature-by-feature

Matrix below grades on the *commercially visible* capability, not internal sophistication. A "yes" means the capability is demonstrable to a buyer; a "partial" means it exists but isn't productized enough to close a deal; a "no" means absent.

| Capability | Prosaic | Arria | Yseop | AX / axite | Retresco | United Robots | RosaeNLG (archived) |
|---|---|---|---|---|---|---|---|
| Deterministic output | **Yes** | Yes (deterministic mode) | Yes | Yes | Yes | Yes | Yes |
| Faithfulness scoring / hallucination gate | **Yes (rare)** | Partial (QA tooling) | Yes (compliance checks) | Partial | No visible | No visible | No |
| Discourse awareness (REG, Centering, RST) | **Yes (research-grade)** | Yes | Yes | Partial | Partial | Partial | Partial |
| Clause aggregation / gapping | Yes | Yes | Yes | Partial | Partial | Partial | Yes |
| Multi-language | EN/ES/DE (3, extensible) | ~20+ claimed | Multi (pharma-focused) | 20+ (strong) | Multi (German strong) | Multi (Nordic strong) | Multi (Pug-based) |
| Pharma / life sciences domain pack | **No** | Some | **Yes — category leader** | No | No | No | No |
| Finance / regulatory domain pack | No | Yes | Partial | No | No | Partial | No |
| E-commerce domain pack | No | Some | No | **Yes — category leader** | Yes | No | Partial |
| Sports / betting domain pack | No | Some | No | Some | Yes | Yes | No |
| Hosted platform / Studio UI | **No** | Yes (Arria Studio) | Yes (Copilot) | Yes | Yes | Yes | No |
| Non-developer authoring surface | No | Yes | Yes | Yes | Yes | Yes | No |
| BI integrations (Power BI, Tableau, Qlik) | No | **Yes** | Partial | Partial | No visible | No | No |
| On-prem / data residency | **Yes (trivially)** | Yes | Yes (enterprise) | Partial | Yes | Partial | Yes |
| WASM / embedded / edge | **Yes** | No | No | No | No | No | Partial |
| Audit log / change tracking / versioning | No (gap) | Yes | **Yes (pharma-grade)** | Partial | Partial | Partial | No |
| Compliance certifications (SOC 2, HIPAA, GDPR) | **No (blocker)** | Yes | Yes | Partial | GDPR | GDPR | No |
| Published faithfulness benchmark | No (gap) | No | No | No | No | No | No |
| MIT/Apache OSS | **Yes** | No | No | No | No | No | Yes (archived) |
| Rust / memory-safe runtime | **Yes (rare)** | No (Java) | No | No | No | No | No (Node) |

**What the matrix says.**

Prosaic's engineering is at or above parity with the commercial vendors on the deterministic-NLG core — discourse awareness, aggregation, REG, multi-language. In some places (faithfulness scoring, WASM, memory safety, `no_std`) it is ahead of every commercial vendor in this list. The README's claim to research-grade NLG is defensible.

Prosaic's product-surface is at zero. No hosted Studio, no authoring UI, no integrations, no domain packs in the verticals where money exists, no audit log, no compliance posture. A buyer in pharma or finance today cannot purchase anything. That is the gap.

The commercial asymmetry is stark: prosaic has built the hard 60% (the engine), and the next 40% (packaging, domain content, compliance, integrations, hosting) is what buyers actually pay for.

---

## 4. Gaps to close to be commercially competitive

Ordered by which gaps gate revenue versus which gaps are "nice to have."

### Revenue-gating (must ship within 6–12 months of starting)

1. **Pick one vertical and build the domain pack.** "Code / git / PR / release" vocab is a perfect developer-marketing surface but not a buyer. The three candidates are pharma clinical writing, financial regulatory reporting, and EU-AI-Act-compliant e-commerce. Pharma has the highest ACV and the hardest moat; e-commerce has the most accessible ACV and the lowest sales friction; finance is in between.
2. **Audit log and versioning.** Every regulated buyer's first RFP question is "can I trace the output back to the input data and template version, and prove it on demand?" Prosaic has `RenderExplanation` — an excellent starting point — but it needs persistence, change tracking on templates, and reproducibility attestation across runs.
3. **Hosted Studio / authoring surface.** Non-engineers in pharma ops, legal, and compliance write the templates. A folder-of-files CLI (`prosaic-project`) is the right foundation but is not what a medical writer uses. The commercial product is a web app on top of `prosaic-project` that edits templates, runs fixtures, diffs outputs, and requires approvals before deploy. This alone is 6 months of work for a small team.
4. **SOC 2 Type 1 (minimum), then HIPAA / GDPR posture.** No enterprise buys without it. Budget $40–80K and 4–6 months.
5. **Published benchmarks.** Pick a faithfulness benchmark (ToTTo, WebNLG, or the E2E NLG Challenge) and publish prosaic's scores against SimpleNLG, a mid-sized LLM, and ideally a trial of Arria/Yseop. Without this, every sales conversation starts from "prove it."

### Deal-expanding (must ship within 12–24 months)

6. **BI integrations.** Narrative captions for Power BI, Tableau, Qlik. This is where Arria makes most of its money and where prosaic could plausibly land co-sell partnerships.
7. **Second and third language corpus validation.** Spanish and German grammar ship the primitives but haven't been stress-tested on real pharma or financial corpora. Hire or contract a native-speaker linguist for each target language and run a 500-sentence evaluation.
8. **SAP / Salesforce / Veeva / Workday connectors.** Pharma lives in Veeva; finance lives in SAP; HR lives in Workday. Even shallow connectors (read-only, CSV-plus-webhook) unlock sales motions that are otherwise impossible.
9. **Document-plan export to Word / PDF with tracked changes.** Pharma medical writers live in Word with tracked changes. The `DocumentPlan` output has to land there natively.

### Strategic, not blocking

10. **Deeper REG and Centering on non-English.** Research-grade work, reputation-building, probably a publication. Doesn't close a deal but widens the moat.
11. **Plugin marketplace for domain packs.** Long-term, if prosaic becomes a platform, third parties writing vocab packs for legal, insurance, etc. is the winning motion. Not year 1.

### What to explicitly *not* build

- An LLM layer. Every competitor is doing this and it muddies the positioning. Prosaic's market is "we never hallucinate because we literally can't." Adding an LLM layer destroys that story. A customer who wants LLM fluency can pipe prosaic's output through their own LLM — that should be a documented pattern, not a product.
- A second programming language port. Rust + WASM covers the deployment surface; a Python SDK that calls the WASM binding is sufficient. Rewriting the engine in Go or Python is a trap.
- A better sentence-length fluency heuristic. Prosaic's output is already more-than-good-enough for regulated contexts where buyers would rather have stiff-but-correct than natural-but-unverifiable.

---

## 5. Pricing structure — open-core + paid tiers

Given the open-core direction, the architectural cut should be:

### What stays free, OSS, MIT/Apache

- `prosaic-core`, all three grammar crates, `prosaic-derive`, `prosaic-wasm`, `prosaic-cli`, `prosaic-tracing`, `prosaic-project` (the folder format).
- All existing vocab modules (`code`, `git`, `pr`, `release`).
- Local faithfulness scoring.
- Documentation, examples, community support via GitHub.

This keeps prosaic attractive to developers, keeps the moat in the form of adoption and mindshare, and matches the actual competitive landscape — RosaeNLG's archival is the cautionary tale, and a strong OSS core is what keeps prosaic from repeating it.

### What becomes paid

**Prosaic Studio (hosted).** Web app for authoring, testing, diffing, deploying templates and vocab packs. Role-based access. Approval workflows. Template version history. Faithfulness dashboards. SSO.

**Enterprise vocab packs.** Pharma clinical (CSR sections, patient narratives, regulatory submission boilerplate). Financial regulatory (10-K/10-Q narrative sections, Basel III disclosure language, MiFID II explanations). EU-AI-Act-compliant e-commerce. These are the commercial crown jewels — they take months of linguist + domain-expert work and are not reasonable for buyers to build themselves.

**Audit & compliance layer.** Persistent audit log, SOC 2 attestation, HIPAA BAA, GDPR DPA, data residency controls, customer-managed encryption keys. Sold as an upgrade, not unbundled.

**Managed hosting / SLAs.** 99.9% SLA, dedicated instances, private cloud deployment, on-prem installation support.

**Connectors.** Veeva, SAP, Salesforce, Tableau, Power BI, Workday. Each is paid, each is an upsell lever.

**Support.** Named CSM, response-time SLAs, onboarding, template migration services.

### Tier structure

Three tiers + an OSS-only tier. Numbers are opening positions — all enterprise pricing moves in negotiation, and the AX-Semantics pull-back from published tiers is a reminder that sales-led pricing dominates in this category.

| Tier | Target | Price | What's in it |
|---|---|---|---|
| **Community** | Developers, OSS projects, non-commercial | Free | Full engine, grammars, all existing vocab. Community support. MIT/Apache. |
| **Team** | Dev teams inside commercial orgs, startups | **$500 / mo** (or $5K / yr) | Studio hosted for up to 5 editors, 1 vocab pack of choice, email support, SOC 2 Type 1. No compliance features. |
| **Business** | Mid-market (e-commerce, content platforms, fintech startups) | **$2,500 / mo** (or $25K / yr) | Up to 20 editors, 3 vocab packs, audit log, GDPR DPA, 1 connector included, SLA 99.5%, 1 named support contact. |
| **Enterprise** | Pharma, banks, healthcare, regulated verticals | **Starting at $75K / yr**, typical deal $120–250K | Unlimited editors, all vocab packs, full audit + HIPAA + SOC 2 Type 2, data residency, dedicated instance, private cloud / on-prem, 99.9% SLA, CSM, onboarding. Priced per volume of rendered output + integrations + deployment mode. |

**Why these numbers.**

The **$500/mo Team tier** is deliberately priced as a credit-card purchase, below the usual procurement threshold. This is what AX Semantics used to do at $279/mo and appears to be retreating from; there is a gap in the market here. It won't generate serious revenue but will generate pipeline — teams that adopt at Team grow into Business.

The **$2,500/mo Business tier** is the sweet spot that essentially no rule-based NLG competitor serves well. Arria and Yseop skip mid-market because their sales cost exceeds the ACV. axite is the only credible option and has moved upmarket. If prosaic can close self-serve or inside-sales deals at this tier, there's a genuine land-and-expand motion.

The **$75K–$250K Enterprise tier** benchmarks below Arria ($100K+) and Yseop ($150K+) on purpose — not as a race to the bottom but because (a) prosaic's compliance story needs time to mature and buyers will discount for a newer vendor, and (b) the deployment flexibility (on-prem, WASM, Rust) allows a lower cost basis. Over time this should move up as the vocab packs deepen and case studies accumulate.

### Revenue modeling

Conservative scenario (solo founder, 24 months, one vertical wedge in e-commerce or finance):

- Community: thousands of users, $0 revenue, pipeline-only
- Team: 20 paying accounts × $500/mo = **$120K ARR**
- Business: 6 accounts × $25K = **$150K ARR**
- Enterprise: 1 account × $100K = **$100K ARR**
- **Total ~$370K ARR at month 24.** Plausible solo-founder outcome.

Aggressive scenario (2–4 person team, focused pharma or financial wedge):

- Team: 40 accounts × $6K = **$240K ARR**
- Business: 20 accounts × $25K = **$500K ARR**
- Enterprise: 5–8 accounts × $150K = **$750K–$1.2M ARR**
- **Total $1.5–2M ARR at month 24.** Requires funded team and vertical specialization.

These are credible ranges, not projections. The enterprise number in either scenario is the hardest to hit — each enterprise deal is a 6–12 month sales cycle with security reviews, pilots, and procurement that a small team can only run two or three of at a time.

---

## 6. Go-to-market recommendation

If you decide to commercialize, the path that matches the evidence is:

**Pick e-commerce as the wedge, not pharma.** Pharma has higher ACVs but takes 18–24 months to land a first reference customer; sales cycles are long, the buyer is hard to reach without existing relationships, and Yseop is dug in. E-commerce has faster cycles, a clearer compliance hook (EU AI Act product-description traceability), and AX Semantics has just pulled back from publishing mid-market pricing — there is an opening. Land e-commerce first, use the revenue to fund the pharma move in year 2–3.

**Lead with faithfulness, not naturalness.** Every demo should show a side-by-side: prosaic's output with `assert_faithful!` passing, next to an LLM's output with a subtle hallucinated number. That is the only positioning that survives contact with an LLM-native buyer.

**Ship the Studio in year 1.** Without a non-developer authoring surface, prosaic is a developer library, and developer libraries don't have a commercial model above consulting revenue. The Studio doesn't need to be beautiful; it needs to let a marketing manager edit a template, preview output on sample data, and request approval from a reviewer.

**Publish a benchmark paper.** A 10-page technical report comparing prosaic on WebNLG, ToTTo, and a custom e-commerce corpus against SimpleNLG, GPT-4o, and Claude would be the single highest-leverage marketing asset. It would also give prosaic a defensible fluency claim independent of marketing copy.

**Don't raise venture money yet.** At $300K–$1.2M ARR potential, this is either a capital-efficient bootstrap or a research-tools-for-enterprise play more suited to a grant + small angel round than a Series Seed. Raising at current market-size signals would either drive valuation too low or set growth expectations that aren't realistic for this niche.

---

## 7. Go / no-go framing

**Reasons to go.** Prosaic's faithfulness scoring + deterministic core is a real wedge that the market is actively paying for in pharma and will increasingly pay for in finance and regulated e-commerce as the EU AI Act enforcement ramps through 2027. The OSS core gives you adoption leverage. Rust + WASM + `no_std` gives you a deployment story no competitor matches. The competitive field is thin and getting thinner (RosaeNLG archived, Arria distressed, Narrative Science absorbed).

**Reasons to hold.** The market is boutique — $150–300M addressable, not $2B. Enterprise sales require someone on the founding team with pharma, finance, or retail-tech relationships; without that, enterprise ARR is structurally out of reach. Compliance certification ($50–80K), Studio build (6 months of engineering), and one vocab pack (3–4 months of linguistic + domain work) is ~$300K of investment before the first Enterprise dollar. And the 24-month revenue ceiling without a funded team is ~$400K ARR — real money, but not a venture outcome.

**Reasons not to go at all.** If the goal is a venture-scale outcome ($50M+ ARR), rule-based NLG is the wrong category. The money is in LLM orchestration, eval tooling, or AI safety infrastructure — all of which prosaic's existing code could be repositioned toward, at the cost of the current identity.

The honest call is: **prosaic is a very good bootstrapped product business and a mediocre venture business.** If you want a $1–3M ARR niche vendor in 3–5 years, the path exists and the engine is already 60% built. If you want anything bigger, you either need to expand the positioning (faithfulness tooling for LLM pipelines, not just rule-based NLG) or pick a different problem.

---

## 8. One-page summary for sharing

**Market:** Rule-based NLG is a $150–300M specialty market in 2026, consolidated into three defensible niches — pharma documentation (Yseop, growing), automated local journalism (United Robots, stable), and e-commerce product content (AX Semantics, vulnerable). The broad-enterprise position (Arria) is failing.

**Prosaic's position:** Engineering is at-or-above commercial parity on the deterministic NLG core, and ahead of every named competitor on faithfulness scoring, WASM/embedded support, and Rust-memory-safety posture. Product surface is at zero — no Studio, no compliance posture, no vertical vocab packs, no integrations.

**What it would take:** 12–18 months of focused work to ship Studio + SOC 2 + one vertical vocab pack + BI integrations. $300–500K of investment or equivalent founder time.

**Pricing proposal:** Open-core OSS; Team $500/mo; Business $2,500/mo; Enterprise $75K+/yr. Sweet spot is the $2,500/mo Business tier where no competitor serves well.

**Revenue expectation:** $300K–$400K ARR at 24 months (solo); $1.5–2M ARR at 24 months (funded team, vertical wedge).

**Recommendation:** Commercially viable as a bootstrapped niche business in e-commerce compliance NLG. Not venture-scale in its current framing.

---

## Sources

- [NLG Market Report — MarketsandMarkets](https://www.marketsandmarkets.com/Market-Reports/natural-language-generation-market-14328817.html)
- [NLG Market — Fortune Business Insights](https://www.fortunebusinessinsights.com/natural-language-generation-nlg-market-105880)
- [Arria NLG — NZ Herald / Newsroom coverage of IRD liquidation & wage dispute](https://newsroom.co.nz/2025/11/05/ai-company-owes-70k-in-unpaid-wages-after-narrowly-avoiding-liquidation/)
- [IRD seeks liquidation of Arria company — NBR](https://www.nbr.co.nz/business/ird-seeks-liquidation-of-arria-company/)
- [Yseop €10M Growth Funding — Claret Capital Partners, Sept 2025](https://yseop.com/news-and-press-releases/claret-capital-partners-announces-e10m-growth-funding-in-yseop-to-scale-generative-ai-platform-for-life-sciences-and-pharma/)
- [axite pricing page (AX Semantics)](https://axite.io/en/pricing-and-axite-plans)
- [Automated Insights / Wordsmith current status](https://automatedinsights.com/wordsmith-2/)
- [Tableau / Salesforce Narrative Science acquisition — TechTarget](https://www.techtarget.com/searchbusinessanalytics/news/252511067/Tableau-completes-acquisition-of-Narrative-Science)
- [RosaeNLG GitHub (archive notice)](https://github.com/RosaeNLG/rosaenlg)
- [SimpleNLG GitHub](https://github.com/simplenlg/simplenlg)
- [United Robots FAQ / pricing model](https://www.unitedrobots.ai/about/faq)
- [Arria NLG G2 Reviews](https://www.g2.com/products/arria/reviews)
