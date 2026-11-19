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
