# Design

The bank reuses nine established factory identities: Singing Lead, Evolving Pad, Ensemble Strings, Breath, Sequence Bass, Interlocking Sequences, Echo Sequence, Composition and Sound Palette Piece. Three gaps receive new identities: Round Keyboard Bass, Glass Keys and Slow Horizons. The existing percussion, simple sequence, recipes, expressive examples and studies remain accessible.

Purpose filters are views of the existing keyboard/clock flags: keyboard plus clock is Performances, clock alone is Sequences, otherwise Sounds. Existing role categories remain exact filters. Search also includes the derived purpose, name, description and tags. Factory/User, favorites, recent history and saved user metadata retain their existing contracts. This avoids adding a competing metadata schema or tagging arbitrary module lists as musical categories.

Interlocking and Echo retain every existing module ID; new macro modules are appended after each original graph. Echo still has delay 16 and right mixer 17. Four real banks provide Pulse, Lift, Sparse and Rest. Echo adds deterministic sequencer probability rather than recorded variation. Other original voices retain their sound and gain defaults. New voices use the existing recipe graph, real velocity multiplication and conservative post-voice gain. Slow Horizons uses three tuned oscillators, a divided clock, a real four-bank sequencer, ladder filter, envelope and reverb. Its parallel minor chords change every two beats at 72 BPM.

Macros route directly to registered parameters. Pins, CC maps, bank launches and cues use existing mechanisms. A short authored guide is a label on the first macro module, rendered in Perform and embedded in complete patch state. Browser descriptions carry the same text. A saved project never consults a subsequent library edit to reconstruct its controls or instructions. Existing route explanations reveal each macro's concrete targets.

Loading safety follows the existing browser replacement and host ownership path: PreviewStop, fresh graph, cleared launches/edit banks/takeover, sequenced load stopped. No callback loading, new DSP runtime, automation identity or transport behavior is introduced. Host mode continues to give REAPER ownership; Free mode exposes the clock's buttons.

Reproduction sources are the factory-bank test/support files and the existing interlocking/echo builders. Metadata regeneration remains in the existing library writer. Run the documented writer sequence to rebuild the exact content, then focused tests and audio evidence. Factory IDs are directory paths and are not renamed.

Evidence separates scripted virtual input, audio measurements and actual application views from owner listening and physical performance. Ordinary REAPER render nondeterminism, the retained full warm-up policy, dense editor/backend errors and strict host-profile assumptions are inherited limits.
