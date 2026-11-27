# baseview

A low-level windowing system geared towards making audio plugin UIs.

`baseview` abstracts the platform-specific windowing APIs (winapi, cocoa, xcb) into a platform-independent API, but
otherwise gets out of your way so you can write plugin UIs.

Interested in learning more about the project? Join us on [discord](https://discord.gg/b3hjnGw), channel `#baseview`.

## Contributing

Contributions are very much welcomed! As long as they comply to the policy and licensing requirements
below.

### AI policy

The general [AI policy of the RustAudio Community](https://rust.audio/community/ai/) applies to this repository. Please
ensure compliance to these rules before submitting your contribution to this project.

## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Baseview by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

Vendored for kabl from https://github.com/RustAudio/baseview rev f106a5190f3a5861cacba7cab9ceb4f9532ebdfe. Only change: `with_dummy_window` in `src/wrappers/win32/window.rs` destroys its dummy window.
