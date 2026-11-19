module main

from ensures import bounded.clamped


----------------------------------------------------------------------------
Keeps a number in range, and does nothing with it.
----------------------------------------------------------------------------
function main():
    kept = clamped(7)
