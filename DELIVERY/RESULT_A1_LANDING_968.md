# RESULT — A1 landing 968-name reconciliation (post-push)

- landed branch: `task/simpl-gaps-a1-land` -> `origin/main`
- landed head: `9c2a0355b37b3bb0c61a7a0f0ae2a552e7f35579`
- base: `5a3d1d21c7cd7aeb013a5e491fd3b5428bc22fe6` (fast-forward, 8 commits A1a..docs)
- tree: Rust 1.75.0 release build; Cargo.lock v3
- binary sha256: `e2a10964b076d986a1cdcdf6f8a5d04f505f74492087e430528ea4c4c789cdeb`
- corpus: `/content/sa_all/sci/demos/rosetta` (968 names)

## rc buckets (fresh vs RESULT_A1_STAGE2_REGSTATE)

| bucket | fresh count / sha256 | expected count / sha256 | match |
|---|---|---|---|
| rc=0 | 962 / `830304a687cf6de03c1cb9650ba7e51051906a577918bb48a762e87b980e2922` | 962 / `830304a687cf6de03c1cb9650ba7e51051906a577918bb48a762e87b980e2922` | True |
| rc=1 | 6 / `8f89c17e76e91f9538cebcdd5bbca74b00907be77ff8f0d2baace7adeb254d55` | 6 / `8f89c17e76e91f9538cebcdd5bbca74b00907be77ff8f0d2baace7adeb254d55` | True |
| all 968 | 968 / `25a6169a30d015f53038f4185f517d1e8ecaef453862f7e8209279569de1cc7c` | 968 / `25a6169a30d015f53038f4185f517d1e8ecaef453862f7e8209279569de1cc7c` | True |

rc distribution = [(0, 962), (1, 6)]
pass->fail = 0 ; fail->pass = 0 (rc buckets per-name identical to RESULT)

rc=1 negative names (six, verbatim): 205_pkg_cyclic_dependency_reject 207_pkg_multiple_versions_conflict 226_mod_cyclic_import_detect 227_mod_shadowing_prevention 243_contract_sig_mismatch_link 502_bare_crossdir_import_reject

Verdict: 968 name-set reconciliation consistent with RESULT_A1_STAGE2_REGSTATE.md.
