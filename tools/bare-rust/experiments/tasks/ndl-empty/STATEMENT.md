`normalized_damerau_levenshtein("", "")` returns NaN. It should be 1.0 (two empty strings are identical), like `normalized_levenshtein` does. Every other input is fine.
