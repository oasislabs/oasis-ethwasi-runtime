#![no_std]

extern crate owasm_std;

// H256 start address prefix for sequential reads.
static SEQ_DATA_LOCATION: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn combine_address(addr: [u8; 32], lsb: u64) -> [u8; 32] {
    let mut loc = addr;
    loc[31] = lsb.to_be_bytes()[7];
    loc[30] = lsb.to_be_bytes()[6];
    loc[29] = lsb.to_be_bytes()[5];
    loc[28] = lsb.to_be_bytes()[4];
    loc[27] = lsb.to_be_bytes()[3];
    loc[26] = lsb.to_be_bytes()[2];
    loc[25] = lsb.to_be_bytes()[1];
    loc[24] = lsb.to_be_bytes()[0];

    loc
}

#[owasm_abi_derive::contract]
trait Readwrite {
    fn constructor(&mut self) {}

    /// Reads data stored at SEQ_DATA_LOCATION..SEQ_DATA_LOCATION+num_locations and returns the
    /// last value. This is executed num_repetitions times.
    //    #[constant] // TODO: This directive yields "no contract code at given address" in go client :(
    fn read_seq(&mut self, num_locations: u64, num_repetitions: u64) -> Vec<u8> {
        let mut read_val: Vec<u8> = Vec::new();

        for _ in 1..=num_repetitions {
            for i in 1..=num_locations {
                let addr = &H256::from(combine_address(SEQ_DATA_LOCATION, i));
                read_val = owasm_ethereum::get_bytes(addr).unwrap();
            }
        }

        return read_val;
    }

    /// Writes pattern to SEQ_DATA_LOCATION..SEQ_DATA_LOCATION+num_locations. This is executed
    /// num_repetitions times.
    fn write_seq(&mut self, pattern: Vec<u8>, num_locations: u64, num_repetitions: u64) {
        for _ in 1..=num_repetitions {
            for i in 1..=num_locations {
                let addr = &H256::from(combine_address(SEQ_DATA_LOCATION, i));
                owasm_ethereum::set_bytes(addr, &pattern).expect("Error writing to location");
            }
        }
    }

    /// Reads data stored at num_locations pseudo-random locations based on seed and returns the
    /// last record. This is executed num_repetitions times.
    //    #[constant] // TODO: This directive yields "no contract code at given address" in go client :(
    fn read_rand(&mut self, seed: u64, num_locations: u64, num_repetitions: u64) -> Vec<u8> {
        let mut read_val: Vec<u8> = Vec::new();

        for _ in 1..=num_repetitions {
            let mut loc = SEQ_DATA_LOCATION;
            for i in 1..=num_locations {
                loc[(seed * i) as usize % loc.len()] = (seed * i % 256) as u8;
                read_val = owasm_ethereum::get_bytes(&H256::from(loc)).unwrap();
            }
        }

        return read_val;
    }

    /// Writes pattern to num_locations pseudo-random locations based on seed. This is executed
    /// num_repetitions times.
    fn write_rand(
        &mut self,
        seed: u64,
        pattern: Vec<u8>,
        num_locations: u64,
        num_repetitions: u64,
    ) {
        for _ in 1..=num_repetitions {
            let mut loc = SEQ_DATA_LOCATION;
            for i in 1..=num_locations {
                loc[(seed * i) as usize % loc.len()] = (seed * i % 256) as u8;
                owasm_ethereum::set_bytes(&H256::from(loc), &pattern)
                    .expect("Error writing to random location");
            }
        }
    }
}
