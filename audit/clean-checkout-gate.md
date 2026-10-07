# clean-checkout gate — row 39

date: 2026-09-24
revision under test: `spark` 400cd9a178792d02ec577ae74fc728fe6ee49ecd (origin/main)

## method

`spark` has one direct path dependency, `file` (`../file`, no version
pin), which transitively pulls `hemera` (`../hemera/rs`, no version
pin). The owner's local `file` checkout matches `origin/main` exactly
(fc666a5, clean), so it was used as-is; the local `hemera` checkout
carries unpushed commits ahead of `origin/main`, so per row 39's own
wording — against the default branches of its siblings — the check
substituted a fresh detached worktree of `hemera` at
`origin/main` (23f3bbcff910ea6d504ceb505680a539260869da) in place of
the owner's drifted local tree, the same method file's own row-39 slice
used (file#9).

## result

```
$ cargo check --tests --workspace
    Checking cyber-hemera v0.3.1 (hemera/rs @ origin/main)
    Checking cyber-file v0.1.0 (file @ origin/main)
    Checking cyber-spark v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

$ cargo test --workspace
running 3 tests
test tests::opaque_has_no_spark ... ok
test tests::png_is_image_spark ... ok
test tests::text_resolves_and_opens ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`spark` passes the clean-checkout gate today: no dead path, no version
pin behind `file`'s or `hemera`'s origin defaults, no crate present
only in an owner's working tree.

## remains

this closes `spark`'s own slice of row 39. the row stays open until
every repo the sweep found broken has its own pin/fix PR merged;
`spark` was not on the sweep's broken list and this measurement
confirms why. `spark`'s own row-16 render work (7 open PRs on image
kinds) is unaffected by this audit.
