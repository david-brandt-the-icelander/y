module main

from compile_time import tables.table_rules, tables.words_of, tables.square_of


----------------------------------------------------------------------------
Reads a word list the build made, and a square from a table it made.

fails list_error.empty: Never: the build made the list, so a list without words would not have built.
----------------------------------------------------------------------------
function main():
    words = compile words_of(table_rules.words)
    nine = square_of(3)
