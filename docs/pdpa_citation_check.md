# PDPA section citations: checked against the Act

> Written 2026-10-08 (session `event-checkin-c0`) for plan 037 §2, the owner
> item "legal review first; text unchanged until then". **This is not legal
> advice and not the legal review.** It checks every section number we cite
> against the text of the Act, so the reviewer starts from a sourced list.
> No catalog text was changed.

**Source:** Personal Data Protection Act B.E. 2562 (2019), Government Gazette
No. 136 Chapter 69 Gor, 27 May 2019. Read in the unofficial English
translation:
- the KMUTT copy (`cc.kmutt.ac.th/Files/Act Eng/personal-data-protection-act-2019-en.pdf`);
- the MSU copy (`pdpa.msu.ac.th/wp-content/uploads/2022/05/EN-PDPA-2019.pdf`).

Both copies have the same text and the same section lines. Only the Thai
text is authoritative. The reviewer should confirm against it, but the
section numbers are the same in both languages.

## What each cited section actually says

| § | Subject in the Act |
|---|---|
| 19 | Consent: written or electronic, purpose stated, kept separate from other matters |
| 20 | Consent of minors and incompetents |
| 23 | Notice to the data subject at or before collection |
| 24 | Lawful bases other than consent; (3) is **performance of a contract** with the data subject |
| 26 | Sensitive data (race, health, biometrics, …) |
| 28 / 29 | Cross-border transfer; 29 is transfer inside an affiliated group under a certified policy |
| 33 | **Right to erasure**, its four grounds, and its exceptions (para 2) |
| 37 | Controller duties: (1) security, (3) an erasure system, (4) breach notice to the Office within 72 h, (5) a representative in Thailand |
| 38 | Who is exempt from appointing the s.37(5) representative |
| 39 | Record of processing (RoPA) |
| 40 | Processor duties; para 3 is the controller–processor agreement |
| 41 | Data Protection Officer |

## User-facing citations (catalogs `locales/{en,th}/privacy.json`)

| Key | We say | Finding |
|---|---|---|
| `basis_body` | consent, Section 19; plus contract performance | **Right.** Consent is s.19. The contract basis is s.24(3), which the text names without a number. |
| `chain_note` (`/privacy` §5) | "Section 37 (technical impossibility exemption)" | **Wrong.** s.37 lists controller duties. The Act has no "technical impossibility" exemption. The erasure exceptions in s.33 para 2 (repeated in s.37(3)) are free expression, s.24(1)/(4), s.26(5)(a)/(b), legal claims and legal compliance. This is not a renumbering: the reviewer has to decide what basis, if any, covers wallet addresses and signatures that cannot be erased from the chain. |
| `data.deletion_body` (`/data-privacy`) | erasure, "Section 29" | **Wrong number.** Erasure is s.33 (s.29 is about transfers inside a group). |
| `data.deletion_body` | "Section 38 — contract performance exemption" | **Wrong number, and the framing needs review.** s.38 is about the representative exemption. Contract performance is s.24(3), but s.33 para 2 does not list it as an exception to erasure. Read plainly, the hold works because no s.33 ground is met while the event is upcoming. The data is still needed under the contract, so ground (1) does not apply, and ground (2) needs "no legal ground" for the processing. That reading is for the reviewer to confirm. |

EN and TH carry the same numbers (`มาตรา 37`, `มาตรา 29`, `มาตรา 38`), so
both change together.

## Internal docs (not user-facing; listed, not edited)

| File | We say | Finding |
|---|---|---|
| `docs/pdpa_breach_procedure.md` | 72 h notice, s.37(4) | Right |
| `docs/pdpa_ropa.md` | RoPA s.39; DPO s.41; processor agreement s.40; transfer s.28/29; bases s.24/s.19 | Right |
| `docs/ux_roadmap.md` PDPA-1 | consent, Section 19 | Right |
| `docs/ux_roadmap.md` PDPA-2 | photos, "Section 20 (sensitive data)" | Wrong: s.20 is about minors; sensitive data is s.26, and an ordinary photo is not sensitive data unless it is used as biometric data |
| `docs/ux_roadmap.md` PDPA-3 | notice, Section 23 | Right |
| `docs/ux_roadmap.md` PDPA-4 | erasure, "Section 29" | Wrong: s.33 |

## Candidate wording for when the review returns

The candidate wording applies only once the reviewer confirms the numbers;
until then the plan keeps the text unchanged. The two `deletion_body`
numbers would become s.33 and s.24(3). `chain_note` needs a decision on the
basis, not a number, so no wording is proposed for it here.
