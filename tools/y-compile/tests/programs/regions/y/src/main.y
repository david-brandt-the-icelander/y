module main

from regions import counting.add_one, counting.counter


----------------------------------------------------------------------------
Adds one to a counter of its own.

system: The root capability, which has nothing to give here, as system.
----------------------------------------------------------------------------
function main(injected system):
    inject(tally = shared.new(counter(count: 0))):
        add_one()
