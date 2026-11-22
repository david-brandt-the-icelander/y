# Results of E4 and E5 (120 runs read: 80 E4, 40 E5)

## E4, by arm

| arm | runs | pass | compiles | hidden tests passed (mean fraction) | attempts (mean) | tokens (mean) | JSON edits, self-reported (mean) | constraint entries changed, graded (mean) | lines changed (mean) | pass among self-reported done |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rust | 40 | 40/40 (100%) | 40/40 (100%) | 1.00 | 1.1 | 223323 | - | - | 11.0 | 40/40 (100%) |
| bare | 40 | 40/40 (100%) | 40/40 (100%) | 1.00 | 1.6 | 269119 | 0.6 | 0.6 | 7.5 | 40/40 (100%) |

## E4, by arm and kind of task

| arm / kind | runs | pass | compiles | hidden tests passed (mean fraction) | attempts (mean) | tokens (mean) | JSON edits, self-reported (mean) | constraint entries changed, graded (mean) | lines changed (mean) | pass among self-reported done |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rust / body | 24 | 24/24 (100%) | 24/24 (100%) | 1.00 | 1.2 | 221600 | - | - | 4.9 | 24/24 (100%) |
| bare / body | 24 | 24/24 (100%) | 24/24 (100%) | 1.00 | 1.2 | 215577 | 0.0 | 0.0 | 2.3 | 24/24 (100%) |
| rust / signature | 16 | 16/16 (100%) | 16/16 (100%) | 1.00 | 1.1 | 225907 | - | - | 20.1 | 16/16 (100%) |
| bare / signature | 16 | 16/16 (100%) | 16/16 (100%) | 1.00 | 2.1 | 349432 | 1.4 | 1.5 | 15.4 | 16/16 (100%) |

## E4, per task (pass over trials: rust | bare)

| task | kind | rust pass | bare pass | rust attempts | bare attempts | bare JSON edits (self) | bare entries changed (graded) |
|---|---|---|---|---|---|---|---|
| error-traits | signature | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.5 | 3.0 | 3.5 |
| fnv-aliases | signature | 2/2 (100%) | 2/2 (100%) | 1.5 | 3.5 | 3.0 | 3.0 |
| fnv-clone | signature | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.0 | 0.0 | 0.0 |
| fnv-const-hash | signature | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.5 | 1.0 | 1.0 |
| fnv-write-order | body | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.0 | 0.0 | 0.0 |
| generic-hamming | signature | 2/2 (100%) | 2/2 (100%) | 1.0 | 2.0 | 1.0 | 1.0 |
| generic-normalized-levenshtein | signature | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.5 | 1.0 | 1.0 |
| hamming-lengths | body | 2/2 (100%) | 2/2 (100%) | 1.5 | 1.0 | 0.0 | 0.0 |
| jaro-single-char | body | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.5 | 0.0 | 0.0 |
| jaro-transpositions | body | 2/2 (100%) | 2/2 (100%) | 2.0 | 1.0 | 0.0 | 0.0 |
| jw-low-similarity | body | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.0 | 0.0 | 0.0 |
| jw-prefix-cap | body | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.0 | 0.0 | 0.0 |
| levenshtein-first-empty | body | 2/2 (100%) | 2/2 (100%) | 1.5 | 1.0 | 0.0 | 0.0 |
| ndl-add | signature | 2/2 (100%) | 2/2 (100%) | 1.0 | 2.0 | 1.5 | 1.5 |
| ndl-empty | body | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.0 | 0.0 | 0.0 |
| nlev-multibyte | body | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.0 | 0.0 | 0.0 |
| osa-transposition | body | 2/2 (100%) | 2/2 (100%) | 1.5 | 1.0 | 0.0 | 0.0 |
| sorensen-dice | signature | 2/2 (100%) | 2/2 (100%) | 1.0 | 4.0 | 1.0 | 1.0 |
| sorensen-repeats | body | 2/2 (100%) | 2/2 (100%) | 1.0 | 1.5 | 0.0 | 0.0 |
| sorensen-whitespace | body | 2/2 (100%) | 2/2 (100%) | 1.0 | 2.0 | 0.0 | 0.0 |

## E5, library variants (E5 tasks only; full = the E4 bare arm on the same tasks)

| variant | runs | pass | compiles | hidden tests passed (mean fraction) | attempts (mean) | tokens (mean) | JSON edits, self-reported (mean) | constraint entries changed, graded (mean) | lines changed (mean) | pass among self-reported done |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| full library (E4 bare) | 20 | 20/20 (100%) | 20/20 (100%) | 1.00 | 1.8 | 288528 | 0.4 | 0.4 | 6.5 | 20/20 (100%) |
| no library files (bare-nolib) | 20 | 20/20 (100%) | 20/20 (100%) | 1.00 | 2.2 | 334874 | 0.4 | 0.4 | 6.5 | 20/20 (100%) |
| library without `unexpressed` (bare-nounexpressed) | 20 | 20/20 (100%) | 20/20 (100%) | 1.00 | 2.1 | 312097 | 0.4 | 0.4 | 6.5 | 20/20 (100%) |

Deltas against the full library (positive = better than the full library; pass rate in percentage points, attempts and tokens as the change):

| variant | d pass (points) | d compiles (points) | d attempts | d tokens | d entries changed |
|---|---:|---:|---:|---:|---:|
| no library files (bare-nolib) | +0.0 | +0.0 | +0.4 | +46346.0 | +0.0 |
| library without `unexpressed` (bare-nounexpressed) | +0.0 | +0.0 | +0.3 | +23569.0 | +0.0 |

### E5 by kind of task

| variant / kind | runs | pass | compiles | hidden tests passed (mean fraction) | attempts (mean) | tokens (mean) | JSON edits, self-reported (mean) | constraint entries changed, graded (mean) | lines changed (mean) | pass among self-reported done |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| full / body | 14 | 14/14 (100%) | 14/14 (100%) | 1.00 | 1.3 | 229113 | 0.0 | 0.0 | 2.3 | 14/14 (100%) |
| no / body | 14 | 14/14 (100%) | 14/14 (100%) | 1.00 | 1.6 | 267445 | 0.0 | 0.0 | 2.5 | 14/14 (100%) |
| library / body | 14 | 14/14 (100%) | 14/14 (100%) | 1.00 | 1.4 | 243515 | 0.0 | 0.0 | 2.3 | 14/14 (100%) |
| full / signature | 6 | 6/6 (100%) | 6/6 (100%) | 1.00 | 2.8 | 427162 | 1.3 | 1.3 | 16.5 | 6/6 (100%) |
| no / signature | 6 | 6/6 (100%) | 6/6 (100%) | 1.00 | 3.8 | 492208 | 1.3 | 1.3 | 15.7 | 6/6 (100%) |
| library / signature | 6 | 6/6 (100%) | 6/6 (100%) | 1.00 | 4.0 | 472120 | 1.3 | 1.3 | 16.3 | 6/6 (100%) |

## Library promises cited (agent self-report, `library_promises_relied_on`)

| arm | runs | runs citing at least one | citations (total) | distinct ids | citations of entries that carry an `unexpressed` clause | ids not in the library file |
|---|---:|---:|---:|---:|---:|---:|
| bare | 40 | 1 | 1 | 1 | 1 | 0 |
| bare-nolib | 20 | 0 | 0 | 0 | 0 | 0 |
| bare-nounexpressed | 20 | 0 | 0 | 0 | 0 | 0 |

Most cited in the full-library arm:

- `std::fmt::Display::fmt`: 1 (has `unexpressed`)

## E5, per task (pass over trials)

| task | full | no library | no `unexpressed` |
|---|---|---|---|
| fnv-aliases | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| fnv-clone | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| fnv-write-order | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| hamming-lengths | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| jaro-single-char | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| levenshtein-first-empty | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| nlev-multibyte | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| sorensen-dice | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| sorensen-repeats | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
| sorensen-whitespace | 2/2 (100%) | 2/2 (100%) | 2/2 (100%) |
