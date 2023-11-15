fn nth_bit(v: u8, b: u8) -> u8 {
    (v >> b) & 1
}

fn nth_flip(v: u8, b: u8) -> u8 {
    v ^ (1 << b)
}

pub fn encode_hamming(v: u8) -> u8 {
    assert!((v & 0xf) == v);
    let d1: u8 = nth_bit(v, 0);
    let d2: u8 = nth_bit(v, 1);
    let d3: u8 = nth_bit(v, 2);
    let d4: u8 = nth_bit(v, 3);

    // even parity
    let p1: u8 = d1 ^ d2 ^ d4;
    let p2: u8 = d1 ^ d3 ^ d4;
    let p4: u8 = d2 ^ d3 ^ d4;
    let p8: u8 = p1 ^ p2 ^ d1 ^ p4 ^ d2 ^ d3 ^ d4;

    p1 | (p2 << 1) | (d1 << 2) | (p4 << 3) | (d2 << 4) | (d3 << 5) | (d4 << 6) | (p8 << 7)
}

pub fn decode_hamming(mut h: u8) -> Option<(u8, bool)> {
    let p1: u8 = nth_bit(h, 0);
    let p2: u8 = nth_bit(h, 1);
    let d1: u8 = nth_bit(h, 2);
    let p4: u8 = nth_bit(h, 3);
    let d2: u8 = nth_bit(h, 4);
    let d3: u8 = nth_bit(h, 5);
    let d4: u8 = nth_bit(h, 6);
    let p8: u8 = nth_bit(h, 7);

    // calculate potential error location (idx + 1, 0 is no error)
    let mut i = 0;
    if (p1 ^ d1 ^ d2 ^ d4) == 1 {
        i += 1
    }
    if (p2 ^ d1 ^ d3 ^ d4) == 1 {
        i += 2
    }
    if (p4 ^ d2 ^ d3 ^ d4) == 1 {
        i += 4
    }

    /* assume we didn't have to fix any bits */
    let mut f = false;

    /* parity error over [7,4] */
    if (p8 ^ d4 ^ d3 ^ d2 ^ p4 ^ d1 ^ p2 ^ p1) == 1 {
        /* parity bit flipped, fix location */
        if i == 0 {
            i = 8;
        }

        /* fix value */
        h = nth_flip(h, i - 1);
        f = true;
    } else if i != 0 {
        return None;
    }

    // read potentially corrected values
    let d1: u8 = nth_bit(h, 2);
    let d2: u8 = nth_bit(h, 4);
    let d3: u8 = nth_bit(h, 5);
    let d4: u8 = nth_bit(h, 6);

    Some((d1 | (d2 << 1) | (d3 << 2) | (d4 << 3), f))
}

#[test]
fn hamming_no_flips() {
    /* check correct operation */
    for i in 0..16 {
        let h = encode_hamming(i);
        let v = decode_hamming(h);

        assert!(!v.is_none());
        let (v, f) = v.unwrap();

        if i != v {
            print!("mismatch: {:b} => {:b} => {:b}", i, h, v);
        }

        assert_eq!(i, v);
        assert_eq!(false, f);
    }
}

#[test]
fn hamming_one_flip() {
    /* check single bit correction */
    for j in 0..8 {
        for i in 0..16 {
            let mut h = encode_hamming(i);
            h ^= 1 << j;
            let v = decode_hamming(h);

            assert!(!v.is_none());
            let (v, f) = v.unwrap();

            /* help debugging */
            if i != v {
                print!("mismatch: {:b} => {:b} => {:b}", i, h, v);
            }

            assert_eq!(i, v);
            assert_eq!(true, f);
        }
    }
}

#[test]
fn hamming_two_flips() {
    /* check double bit error */
    let mut h = encode_hamming(0b0000);
    h ^= 1 << 1;
    h ^= 1 << 2;
    let v = decode_hamming(h);
    assert_eq!(v, None);
}
