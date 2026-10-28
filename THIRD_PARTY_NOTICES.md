# Third-party notices

This notice is a partial inventory, not a complete software bill of materials or legal compliance determination. The repository contains upstream rule lists, modules, scripts, generated material, and Rust dependencies whose provenance and license terms require a broader, continuing review. No unverified license is assigned here.

## Project license boundary

The repository's [LICENSE](LICENSE) contains the GNU General Public License, version 3. It applies to project-authored code only to the extent the relevant copyright holder has authority to license that code under those terms. It does not replace or relicense third-party materials. No project copyright holder or author is asserted by this notice.

## Currently documented upstream licenses

The following source-specific notices and license texts are maintained in [`rulesets/Sources/licenses/`](rulesets/Sources/licenses/):

- `app2smile-MIT.txt` — MIT license text associated with the documented app2smile/rules material.
- `AdGuard-CNAME-MIT.txt` — MIT license text associated with the documented AdguardTeam/cname-trackers material.
- [`README.md`](rulesets/Sources/licenses/README.md) records the relevant upstream sources, use, modifications, and the boundary for resources not redistributed by this repository.

These entries cover the sources described by that file; they do not establish the licenses for every file or every historical contribution in the repository.

## Rust dependencies and other sources

[`create/processor/THIRD_PARTY_LICENSES.txt`](create/processor/THIRD_PARTY_LICENSES.txt) is generated with cargo-about from the locked desktop dependency graph for the configured macOS and Windows targets and the `gui` feature. It includes license text grouped with the crate names and versions that use it. The `about.toml` allow-list only permits declared SPDX license branches; generation fails for any unsatisfied license requirement rather than silently omitting it. This generated dependency inventory does not cover runtime-downloaded modules, rules, or scripts, and is not a complete review of all repository content.

The dependency manifest for `epaint_default_fonts` declares `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`. Its bundled font asset notices are included verbatim in [`create/processor/licenses/epaint_default_fonts/`](create/processor/licenses/epaint_default_fonts/): the SIL Open Font License, Ubuntu Font Licence, Hack/Bitstream Vera notices, and the emoji-icon-font MIT notice plus the egui-icons subset provenance note. These asset-specific terms are not replaced by the crate's general MIT/Apache alternatives.

The locked `libsqlite3-sys` crate has its own MIT license, included in the generated dependency notice. Its bundled SQLite amalgamation has a separate source-level copyright disclaimer and blessing, noted in the generated file; do not confuse that notice with the Rust binding crate's MIT license.

The generated dependency list and asset notices do not establish complete licensing or redistribution clearance for every repository source. Do not infer a license for an unreviewed file from this inventory, and do not treat generation as a full legal compliance determination.

Desktop release packages include this notice, the root license, privacy and security notices, the generated dependency license texts, the currently tracked upstream MIT license texts, and the bundled-font asset notices. The source repository at the exact build commit is identified in each package's `SOURCE.txt`; that pointer is not a promise of indefinite hosting beyond the applicable license requirements.
