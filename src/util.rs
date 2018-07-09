use ekiden_core::error::{Error, Result};
use ethcore_types::log_entry::LogEntry;
use ethereum_types::U256;
use evm_api::{error::INVALID_BLOCK_NUMBER, TopicFilter};
use hex;
use state::{block_by_number, get_latest_block_number};
use std::str::FromStr;

pub fn strip_0x<'a>(hex: &'a str) -> &'a str {
    if hex.starts_with("0x") {
        hex.get(2..).unwrap()
    } else {
        hex
    }
}

pub fn from_hex<S: AsRef<str>>(hex: S) -> Result<Vec<u8>> {
    Ok(hex::decode(strip_0x(hex.as_ref()))?)
}

pub fn to_hex<T: AsRef<Vec<u8>>>(bytes: T) -> String {
    hex::encode(bytes.as_ref())
}

fn check_log_topic(log: &LogEntry, index: usize, filter: &TopicFilter) -> bool {
    match filter {
        &TopicFilter::All => true,
        &TopicFilter::Or(ref hashes) => {
            if log.topics.len() >= index {
                false
            } else {
                let mut matched = false;
                for hash in hashes {
                    if hash == &log.topics[index] {
                        matched = true;
                    }
                }
                matched
            }
        }
    }
}

pub fn parse_block_number(value: &Option<String>, latest_block_number: &U256) -> Result<U256> {
    if value == &Some("latest".to_string()) || value == &Some("pending".to_string())
        || value == &None
    {
        Ok(latest_block_number.clone())
    } else if value == &Some("earliest".to_string()) {
        Ok(U256::zero())
    } else {
        match U256::from_str(&value.clone().unwrap()) {
            Ok(val) => Ok(val),
            Err(_) => return Err(Error::new(INVALID_BLOCK_NUMBER)),
        }
    }
}

// TODO: re-enable
/*
pub fn get_logs_from_filter(filter: &LogFilter) -> Result<Vec<FilteredLog>> {
    let latest_block_number = get_latest_block_number();
    let from_block = parse_block_number(&filter.from_block, &latest_block_number)?;
    let to_block =
        latest_block_number.min(parse_block_number(&filter.to_block, &latest_block_number)?);

    if from_block > to_block {
        return Err(Error::new(format!("{:?}", "Invalid block range")));
    }

    let mut current_block_number = from_block;
    let mut ret = Vec::new();

    while current_block_number <= to_block {
        let block = match block_by_number(current_block_number) {
            Some(block) => block,
            None => break,
        };

        match get_transaction_record(&block.transaction_hash) {
            Some(record) => {
                for i in 0..record.logs.len() {
                    let log = &record.logs[i];

                    let passes_filter = filter.addresses.contains(&log.address)
                        && check_log_topic(log, 0, &filter.topics[0])
                        && check_log_topic(log, 1, &filter.topics[1])
                        && check_log_topic(log, 2, &filter.topics[2])
                        && check_log_topic(log, 3, &filter.topics[3]);

                    if passes_filter {
                        ret.push(FilteredLog {
                            removed: false,
                            log_index: i,
                            transaction_index: 0,
                            transaction_hash: block.transaction_hash,
                            block_hash: block.hash,
                            block_number: block.number,
                            data: record.logs[i].data.clone(),
                            topics: record.logs[i].topics.clone(),
                        });
                    }
                }
            }
            None => {}
        }

        current_block_number = current_block_number + U256::one();
    }

    return Ok(ret);
}
*/
