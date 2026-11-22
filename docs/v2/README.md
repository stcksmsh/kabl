# kabl V2: audio that beats Surge, video that beats Lumen, one modular graph

Date 2026-11-22. Proposal for Kosta; nothing here is an owner decision yet.

| Doc | Question it answers |
|---|---|
| [AUDIO.md](AUDIO.md) | What does kabl's audio side have, what does it lack against Surge XT, and in what order should the gaps close? |
| [VIDEO-CORE.md](VIDEO-CORE.md) | What does synteza really contribute, and how does it become a Lumen-class video engine? |
| [AV-MERGE.md](AV-MERGE.md) | How do the two become one modular instrument: cross-domain cables, joint modules with ports in both domains, and containers that hold both? |

Earlier work this builds on: [av-audit](../av-audit/KABL_V2_PLAN.md). AV-MERGE §10 lists where this plan departs from it.

## One-paragraph summary

kabl's DSP quality is high, but its breadth is narrow: 8 voices, MIDI only, block-rate parameter modulation, few filter types, Linux and CLAP only. The A0–A5 batches close that gap.

synteza contains good shaders and ideas, but it is not yet an engine: offline only, 8-bit textures, one hard-wired look. It becomes a wgpu frame-graph of small typed modules (VIDEO-CORE V0–V6).

kabl's composites are already flat ownership metadata. Containers that span both domains therefore come almost free once ports carry a domain. The real work is:
- a compiler pass that splits the patch into an audio graph and a visual graph;
- a sample-timestamped bridge between them;
- module kinds with two executors (audio and visual).

All three fit kabl's existing seams: cable params, `Step::Cable`-style compiled nodes, revision-gated graph delivery, and state carry.

## Combined size (agent-weeks, one agent, review latency excluded)

| Work | Estimate |
|---|---|
| Audio A0–A4 | 27–38 |
| Video V0–V6 | 21–29 |
| AV merge M1–M7 (glue only) | about 14–20 |

With two agents running in parallel (audio, and video + merge), expect roughly 12–18 months of calendar time.
