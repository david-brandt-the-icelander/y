`levenshtein("", "abc")` returns 0 but must be 3 (the number of insertions), and `generic_levenshtein` does the same with empty sequences. `levenshtein("abc", "")` and the non-empty cases are right.
