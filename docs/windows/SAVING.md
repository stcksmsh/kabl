# Saving on Windows

Every save writes a temporary file next to the target (same directory, so same volume), flushes
it, then replaces the target through `kabl_core::fsutil::replace_file`. That helper is the only
place in the kabl, kabl-ui and kabl-standalone sources that calls `fs::rename` for a save.

## Behaviour

- Linux and macOS: a single `rename(2)`, no retry, as before.
- Windows: `ReplaceFileW` (kernel32, declared by hand, no new dependency), which keeps the
  target's attributes and ACLs. A target that does not exist yet is a plain rename.
- A target that another program has open fails with `ERROR_SHARING_VIOLATION` (32) or
  `ERROR_ACCESS_DENIED` (5). Those two codes are retried with a growing wait, about 0.95 s in
  total, on the thread that saves (UI or worker, never audio). Any other error is returned at once.
- A read-only target is reported as an error without retrying.
- When the budget runs out, or on any final error, the temporary file is removed, the old target
  is untouched. After retries the error names the file and says it is in use by another program
  (sharing violation) or that access was denied (error 5). Log rotation uses `move_file`, which
  keeps the source file on failure.

## Sites that do not use the helper

- `crates/modules/src/code/install.rs` (`.part` then rename, a download install) and
  `crates/ui/src/record.rs` (renames a bad take to `*-INCOMPLETE.wav`): outside this slice's scope.
- `crates/ui/src/composites.rs` publishes with `hard_link` (never overwrites), not a rename.

## Needs a real Windows check

Save a sound and a patch while the target is open in another program: a text editor with the
file locked, an antivirus scan of the folder, a cloud-sync client, a second kabl window. A plain overwrite-save (target exists, `ReplaceFileW`) has not been run on Windows at all yet: check it
first, with no other program involved. Expect the
save to succeed after a short pause, or to fail within about a second with the "in use" message
and no `.tmp` or `.new` file left behind. The Linux tests only simulate the sharing violations.
