# Factory composite bank

Gap G4 in `docs/pivot/composite-catalogue.md`: factory composites need stable identities and
a read-only home next to the factory sounds.

## Ids and versions

- A factory definition's id is `kabl.<name>`, where `<name>` is lowercase ASCII letters, digits
  and `-` (at most 64 bytes). Its version is a positive integer. `kabl.space` v1 is the first
  version of Space.
- Both come from the file's place in the bank, `<name>/v<N>.json`, so they are the same in every
  load and every process. `v01.json` and `v0.json` are ignored, as is anything else that does
  not fit the pattern.
- The root composite inside the file must carry the same definition
  (`"definition": {"id": "kabl.space", "version": 1}`). A file that disagrees with its path is
  refused when inserted, with a message naming both.
- User definitions keep their random ids (`kabl-<pid>-<nanos>-<n>`). Publishing refuses any id
  containing a dot, so a user id can never equal a factory id and publishing can never write
  into a factory definition's folder.

## Read-only

- `composites::publish` refuses a definition whose id starts with `kabl.`: an inserted factory
  instance cannot be published as a new factory version, wherever the user library is.
- The library has no delete or rename for definitions, user or factory. The UI never writes to
  the bank.
- An inserted instance is an ordinary embedded snapshot (D09). It can be edited in the patch;
  its definition stays `kabl.<name>` vN, and it never changes when the bank does. A patch that
  embeds `kabl.space` v1 keeps v1 when the bank later ships v2.

## Where the bank lives

The first that exists of:

1. `KABL_FACTORY_COMPOSITES_DIR`;
2. `<exe>/../share/kabl/composites` (the Linux package layout);
3. `$XDG_DATA_DIRS/kabl/composites` (default `/usr/local/share:/usr/share`);
4. the source checkout's `composites/` (a development build).

`packaging/linux/package.sh` copies `composites/` to `share/kabl/composites` next to
`share/kabl/patches`. With no bank, or an empty one, Insert shows the user library only and
reports nothing.

## Insert

Composites (side rail) → "Insert a library version as an independent embedded instance". When
the bank has definitions, a "Factory (read-only)" list comes first, one button per version
labelled `kabl.<name> vN`, then "Your library", listed by path under the user root. Two
versions of one id are both listed. A user library folder that happens to be named like a
factory id stays under "Your library", labelled by its path; neither list hides the other.

## Adding a definition

1. Build the composite in a patch and publish it (Composites → "Publish a new immutable
   version"), or export it as a package.
2. Copy the file to `composites/<name>/v1.json`, or the next free `vN.json` for a new version.
   Never edit a released version: patches already embed it, and a new behaviour is a new
   version.
3. In the copy, set the root composite's `definition` to `{"id": "kabl.<name>", "version": N}`.
   The root is the composite named by the package's `root` field.
4. Run `cargo test -p kabl-ui --test composites_factory`, and open Composites in the app to
   check that it inserts.
