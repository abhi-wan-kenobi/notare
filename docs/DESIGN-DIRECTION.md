# Notare Design Direction

**Status:** Direction document (pre-implementation) · 2026-07-16
**Thesis:** Notare is a power tool for meetings. The UI should feel like a precision
instrument that *glows when it's alive* — dark, quiet, dense, keyboard-first — with
exactly one theatrical element: the orb. Everything else earns its pixels.

Owner constraints (non-negotiable): dark-first "glowing product" aesthetic (ref:
casaos.zimaspace.com), **no** paper/serif/amber "generic AI look", **no** card-grid
layouts. Speed and function beat decoration.

---

## 1. Reference analysis

### R1 — "NoteWise" AI Meeting Note SaaS landing (Sahil Dobariya / Heloxone)
Light glassmorphic marketing page: organic peach/green gradients, serif-italic
headline accents, spacious "three steps from meeting to clarity" narrative.
- **Take:** the *narrative clarity* — one job stated in one line; the discipline of
  a single centered product surface instead of feature clutter. Useful for the
  website/onboarding copy, not the app.
- **Skip:** almost all of the visuals. Serif italics + soft organic gradients are
  exactly the "generic AI look" we're banned from. Light glassmorphism fights
  legibility in a dense tool.

### R2 — "Meeting Live Transcript" (Ehsan Masoomi) — *the most load-bearing reference*
Neutral greys (#E8E8E8/#0D0D0D) + one red accent (#BD2E2D). Record/Live-Transcript
pill toggle; red "● Recording…" whisper; huge timer with `00:02:` in grey and seconds
in bold black; meeting title under it; speaker rows = avatar + name + timestamp chip
+ transcript body; content fades out at the bottom edge; Pause/Stop float over the
fade.
- **Take:** everything structural. The **weighted timer** (dim hh:mm, bold ss) is a
  perfect "alive" signal with zero animation cost. Speaker-row anatomy, timestamp
  chips, bottom fade-out with floating controls, red reserved *exclusively* for
  recording state. This is our live-transcript blueprint, re-skinned dark.
- **Skip:** italic body text (cute, hurts scanability at density), the acres of
  empty margin — we need 2× the information density.

### R3 — "AI Transcription Voice App" (Sahariar Hossain)
Lilac/violet mobile app (#8F7BF0/#4D3CD8 on #BEC6D8): glowing waveform orb,
concentric pulse rings around the mic, live-dictation text where already-committed
words are grey and the current phrase is dark ("**Today**, I want to tell…"), timer
inside the recording button, waveform scrubber.
- **Take:** the **two-tone dictation text** (finalized vs. in-flight words — maps 1:1
  to our partial/final STT segments), concentric pulse rings as the recording
  idiom, timer-in-the-orb.
- **Skip:** the pastel lilac wash (too soft, reads consumer), oversized tap targets —
  we're a desktop tool, not a phone.

### R4 — "Sphere design for an AI voice assistant" (AmazingUI)
Iridescent glass orb: cobalt-blue liquid core inside a translucent shell, cyan/mint/
violet rim light, specular highlight, floating on a neutral field. Siri-energy.
- **Take:** this **is** the Notare orb — our one allowed piece of theater. Cobalt
  core = accent anchor for the whole palette. Liquid fill level / turbulence can
  encode state (idle / listening / transcribing / enhancing).
- **Skip:** nothing conceptually — but implement as layered CSS/SVG gradients +
  filter blur, not a 3D render or video texture. Must run at 60fps in a tiny
  always-on-top window.

### R5 — "Voice AI interface" (Gleb Kuznetsov / Milkinside)
Ethereal frost-white field; topics ("Apple Meeting", "Weekend plans") as luminous
particles with hairline leader lines; glow brightness = importance.
- **Take:** the principle **glow = information, not decoration**. Hairline leader
  lines and luminous dots are a beautiful idiom for "AI activity" markers in the
  margin (e.g., where the enhancer linked a note to transcript evidence).
- **Skip:** the whole white-mist aesthetic and the near-zero density — it's a
  concept film, not a tool.

**Synthesis:** R2's skeleton + R4's soul, R3's live-text mechanics, R5's glow ethic,
R1's copy discipline. All rendered in the CasaOS register: near-black surfaces,
saturated cool accents, glow used sparingly and meaningfully.

---

## 2. The Notare design language

### Palette — dark-first, "cobalt on graphite"
Glow is a *state channel*: things glow only when live (recording, streaming,
enhancing). A static screen is matte.

| Token | Dark (default) | Light variant | Role |
|---|---|---|---|
| `bg` | `#0B0D12` | `#F4F5F7` | app canvas (blue-black, never pure #000) |
| `surface` | `#12151D` | `#FFFFFF` | panels, sidebar |
| `surface-raised` | `#1A1E29` | `#FFFFFF` + shadow | popovers, floating controls |
| `border` | `#232837` | `#E3E5EA` | 1px hairlines, the primary depth cue |
| `text` | `#E8EAF0` | `#16181D` | primary |
| `text-muted` | `#8B92A6` | `#5C6270` | secondary, committed-transcript words |
| `accent` | `#4D5CFF` (cobalt) | `#3947E8` | actions, links, focus, the orb core |
| `accent-glow` | `#6EE7FF → #A78BFA` | same, 60% α | iridescent rim — *live states only* |
| `rec` | `#FF4D5E` | `#E03546` | recording only. Never reused for errors-in-general. |
| `ok` | `#3DD68C` | `#22A56B` | model verified, saved, connected |

Rules: red = recording, cobalt = interactive, iridescence = AI-is-working. No amber
anywhere. Light theme is the same geometry with glows reduced to subtle shadows —
it exists (R2 proves it works) but dark is the design target; light must never
drive a decision.

### Typography
- **UI:** keep a sans stack but pin it — ship **Inter variable** (or Geist) instead
  of raw `system-ui`, so Linux/Windows stop rendering three different apps.
  Sizes: 13px UI default, 12px meta, 15px editor body. Tight, not cramped.
- **Numerals:** `font-variant-numeric: tabular-nums` mandatory on timers,
  timestamps, token/word counts. The R2 weighted timer (dim `hh:mm`, bold `ss`)
  is a typography trick, not a component.
- **Mono:** one mono face (JetBrains Mono / Geist Mono) for timestamps chips,
  model names, shortcuts, diagnostics.
- **Banned:** serif display faces, italics-as-style (keep italic for editor
  emphasis only), and "Bradley Hand"/CabinSketch anywhere user-visible.

### Surface, depth, glow
- Depth = **1px borders + soft large-radius shadows**, not stacked translucency.
  Blur (`backdrop-filter`) is allowed in exactly two places: the floating widget
  and the command palette.
- Radii: 6px controls, 10px panels, full-round pills for state chips. One family.
- Glow recipe (the only one): outer `box-shadow` 0 0 24–40px of `accent`/`rec` at
  8–15% α + a 1px inside border of the same hue at 40% α. Applied to: recording
  indicator, orb, active "listening" transcript edge, primary CTA on hover. Never
  on static cards, headers, or empty states.

### Motion
- 120–160ms `ease-out` for state changes; 200–240ms for panel slide/fade. Nothing
  slower than 240ms, ever; no springs on text.
- Live transcript: new words fade in at 80ms with a 1-word stagger — no slide, no
  typewriter LARP. In-flight (partial) words render `text-muted`, then "commit" to
  `text` (R3's two-tone trick — it makes STT latency *feel* like intelligence).
- The orb is the one element allowed continuous animation (slow 6–8s hue drift
  idle; amplitude-reactive when listening). Everything else animates only on event.
- Respect `prefers-reduced-motion`: orb falls back to a static gradient + level bar.

### Information density
Power-tool defaults: sidebar rows one line high, metadata inline not stacked,
timestamps always visible, no decorative illustrations on screens that have data.
Empty states get one line of copy + one action, not a hero graphic. Keyboard:
every surface reachable via existing shortcuts + a `⌘K` palette is the north star.
**No card grids** — lists, tables, and split panes only.

---

## 3. Signature elements

### 3a. Live transcript (meeting mode) — the money screen
`session/components/note-input/transcript/` (`screens/listening.tsx`, renderer).
Layout per R2, dark: sticky header = red pulse dot + weighted timer + meeting title
+ device chip; body = speaker rows (avatar/initial, name, mono timestamp chip,
transcript text); bottom 15% fades to `bg` with Pause/Stop floating over it.
Partial segments in `text-muted` → commit to `text`. A hairline cobalt "live edge"
glows at the newest line (R5 glow ethic). Speaker reassignment stays inline. Target:
zero layout shift as words stream — reserve line height ahead of the caret.

### 3b. The orb / floating widget — multi-theme
`meeting-float/host.tsx` + native overlay (`plugins/overlay/`, `plugins/windows/`).
One widget, three user-selectable skins (a settings enum, same state machine):
1. **Sphere** (default; R4): 28–40px iridescent glass orb — cobalt liquid core,
   rim-light ring. States: idle (matte, slow drift) / listening (rim glows, core
   level tracks input amplitude) / transcribing (slow rotation shimmer) /
   enhancing (violet pulse) / error (desaturated + badge).
2. **Bar**: R2-style micro pill — red dot + weighted `mm:ss` + word count; hover
   expands to Pause/Stop/Open. For people who find orbs twee.
3. **Caption bubble**: existing `FloatingTranscriptBubble` restyled — last 2 lines
   of live transcript on `surface-raised` at 92% α + blur.
Implementation: pure CSS gradients/filters (3 layered radial gradients + conic
rim + blur), driven by CSS vars the audio callback updates — no canvas, no video.

### 3c. Notepad-first editor
The Granola move is the product; the editor must read as "quiet notepad", not
"AI app". ProseMirror surface (`note-input/raw.tsx`, `enhanced/editor.tsx`) on bare
`bg` — no page chrome, no card. 15px/1.6 body, 68ch measure, generous only inside
the text column. Enhanced view = same surface; AI-touched blocks get a 2px cobalt
left hairline that fades after review (glow = information). Slash menu + `⌘K`
restyled to `surface-raised`. Headers dim to near-invisible while typing (focus
mode by default, not as a feature).

---

## 4. Redesign priority (mapped to actual surfaces)

| # | Surface | Files | Why this order |
|---|---|---|---|
| 1 | **Tokens + dark theme base** | `apps/desktop/src/styles/globals.css`, `src/styles/dark-theme.css`, `packages/ui/src/styles/globals.css` | Everything inherits it; zero component edits, app-wide payoff day one. |
| 2 | **Live transcript + meeting header** | `note-input/transcript/*`, `session/components/outer-header/` | The money screen; Phase 2 (meeting mode E2E) lands here — design it before the feature hardens. |
| 3 | **Note editor (raw + enhanced)** | `note-input/raw.tsx`, `enhanced/editor.tsx`, `packages/editor/src/note/` | Where users live between meetings. |
| 4 | **Sidebar / shell** | `src/sidebar/*`, `main/shell-*.tsx`, `main/layout.tsx` | Frames every screen; density pass + new tokens. |
| 5 | **Floating widget (orb)** | `meeting-float/host.tsx`, `plugins/overlay/`, `session/components/floating/` | Signature element; ship after transcript so states are real. |
| 6 | **Model manager / AI settings** | `settings/ai/stt/*`, `settings/ai/llm/*`, `ai/hooks/*` | Our headline differentiator (local models, integrity checks) deserves better than forms: verified-✓/CRC states in `ok` green, download progress with glow. |
| 7 | **Settings (rest)** | `settings/general/*`, `settings/personalization/` | Utilitarian restyle, table-like rows. |
| 8 | **Onboarding** | `src/onboarding/*` | Last: fewest visits. Steal R1's one-line narrative; dark, 4 steps max, orb makes its first appearance here. |

---

## 5. Implementation strategy (Tailwind v4, no big bang)

The stack is already right: Tailwind v4 CSS-first `@theme`, shadcn-style HSL vars
(`--background`, `--primary`…), Radix primitives in `packages/ui`, class-based dark
mode. The redesign is a **token swap + component sweep**, not a rewrite.

1. **Phase A — tokens (1 sitting).** Redefine the existing HSL vars in
   `globals.css`/`dark-theme.css` to the §2 palette; add the new ones
   (`--accent-glow`, `--rec`, `--ok`) in `@theme` so `text-rec`/`shadow-glow`
   utilities exist. Add `--shadow-glow-*` and motion-duration tokens. Because every
   shadcn component reads these vars, ~70% of the app re-skins itself. Flip default
   theme to dark in `shared/theme/` + `public/theme-boot.js`.
2. **Phase B — primitives.** Sweep `packages/ui/src/components/ui/*` once: radii,
   border-vs-shadow depth, focus rings (cobalt), sizes down to 13px density. Add
   three new primitives: `GlowDot` (status pulse), `WeightedTimer`, `TimestampChip`.
   Every app surface upgrades for free.
3. **Phase C — surface passes in §4 order.** One surface per PR, using only tokens
   + primitives (no hex literals in components — lint it with the existing eslint
   plugin). Old and new screens coexist without visual whiplash because Phase A
   already unified color.
4. **Phase D — the orb.** New skin components under `meeting-float/`, settings enum
   for widget theme, native-overlay parameters for size/click-through.
5. **Guardrails:** delete `dark-theme.css` overrides as vars absorb them; visual
   snapshots for transcript + editor; keep the 1297-passing frontend test suite
   green per PR; every PR ≤ one surface.

**Do not**: introduce a second component library, add CSS-in-JS, restyle via
per-screen overrides, or fork `packages/ui` styling per window. Tokens are law.
