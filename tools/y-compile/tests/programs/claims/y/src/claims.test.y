module claims


-- The CRC-32 of a known text is the standard one; a native function has no paths, so this test claims none.
test the_checksum_of_a_known_text:
    when:
        crc = checksum("123456789")
    then:
        standard = (crc == 3421780262)


-- A counter holds what it was made with.
test a_counter_holds_its_start:
    when:
        cell = new_cell(7)
    then:
        seven = (count_of(cell) == 7)


-- A secret's length is counted.
test a_secret_s_length
        covers secret_length:
    given:
        secret = sensitive.new("hunter2")
    inject:
        reveal = fake reveal
    when:
        length = secret_length(secret)
    then:
        seven = (length == 7)


-- The smallest of sorted numbers is the first.
test the_smallest_is_the_first
        covers smallest:
    when:
        least = smallest([2, 5, 9])
    then:
        two = (least == 2)


-- One number is short.
test one_number_is_short
        covers sized > measured.short:
    when:
        size = sized([4])
    then:
        short = (size == "short")


-- Six numbers are long.
test six_numbers_are_long
        covers sized > measured.long:
    when:
        size = sized([1, 2, 3, 4, 5, 6])
    then:
        long = (size == "long")


-- Three numbers are neither.
test three_numbers_are_middling
        covers sized > measured.middling:
    when:
        size = sized([1, 2, 3])
    then:
        middling = (size == "middling")
