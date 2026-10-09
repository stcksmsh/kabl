# Review record: promo-demo

Reviewer: a fresh `code-reviewer` subagent, read-only. Reviewed base `f7b83b5`, head `ee13b12`. Subagent review is not Kosta's approval.

| Finding | Severity | Resolution |
|---|---|---|
| `record.sh`: the EXIT trap killed Xvfb but not the background ffmpeg, so a failed drive left an orphan recorder. | Critical | Fixed: the trap is re-set after `REC=$!` and kills both. |
| `post.py`: the t0 correction from the audio was applied even when no lag matched (all correlations -1), shifting captions 1.5 s silently. The docstring said "printed, not corrected". | Medium | Fixed: the script exits when the best correlation is below 0.5; docstring now describes the correction. Checked: a stale start time stops the script, the right one reproduces 0.879. |
| `promo_demo.rs`: the `bass B` stem was rendered and printed twice. | Medium | Fixed. |
| `post.py`: `corr[15]` assumed lag 0 sits at index 15. | Low | Fixed: looked up from `lags`. |
| `record.sh`: `$PATCH` unquoted. | Low | Fixed. |
| `promo_demo.rs`: `render` renders twice (determinism assertion). | Low | Kept, documented in the usage comment: it is the determinism check. |
| `drive.py` `slide`: `getmouselocation` without `check=True`. | Info | Fixed. |
| `post.py`: the `targets.json` path expression looks odd. | Info | Commented. |

The reviewer found nothing wrong in the render path, `morph_at`, the `slide` direction or the pattern parameter indices, and called the README's Verified section broadly honest apart from the "not corrected" wording above (that wording was in the `post.py` docstring; the README says the lag is used).

Recheck: the fixes above are small and were verified by the commands named in the table plus `cargo clippy -p kabl-ui --example promo_demo -- -D warnings`; no second reviewer pass was run.
