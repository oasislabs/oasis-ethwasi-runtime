use error::Error;

pub struct SourceItem {
    pub offset: usize,
    pub length: usize,
    pub file_name: String,
}

impl SourceItem {
    pub fn has_intersection(&self, other: &SourceItem) -> bool {
        let self_start = self.offset;
        let other_start = other.offset;
        let self_end = self.offset + self.length;
        let other_end = other.offset + other.length;

        self.file_name == other.file_name &&
            self_end > other_start && self_start < other_end
    }

    pub fn find_intersection<'a>(&self, others: &'a [SourceItem]) -> Option<(usize, &'a SourceItem)> {
        for (index, item) in others.iter().enumerate() {
            if self.has_intersection(item) {
                return Some((index, item));
            }
        }
        return None;
    }
}

pub enum JumpType {
    FunctionIn,
    FunctionOut,
    Regular
}

pub struct SourceMapItem {
    pub source: SourceItem,
    pub jump: Option<JumpType>,
}

pub fn parse_source(s: &str) -> Result<Vec<SourceItem>, Error> {
    let mut ret = Vec::new();
    let mut last = 0;
    for item in s.split(';') {
        let mut values: Vec<usize> = Vec::new();
        let mut file_name = None;
        for raw in item.split(':') {
            if values.len() > 2 {
                file_name = Some(raw.to_string());
                break;
            }
            if raw.is_empty() {
                values.push(last);
            } else {
                let value = raw.parse()?;
                values.push(value);
                last = value;
            }
        }

        while values.len() < 2 {
            values.push(last);
        }

        if file_name.is_none() {
            return Err(Error::InvalidParams);
        }

        ret.push(SourceItem { offset: values[0], length: values[1], file_name: file_name.unwrap() });
    }
    Ok(ret)
}

pub fn parse_source_map(s: &str, l: &[String]) -> Result<Vec<SourceMapItem>, Error> {
    let mut ret = Vec::new();
    let mut last = 0;
    for item in s.split(';') {
        let mut values: Vec<usize> = Vec::new();
        let mut jump_value = None;
        for raw in item.split(':') {
            if values.len() > 3 {
                jump_value = Some(raw);
                break;
            }
            if raw.is_empty() {
                values.push(last);
            } else {
                let value = raw.parse()?;
                values.push(value);
                last = value;
            }
        }

        while values.len() < 3 {
            values.push(last);
        }

        let file_index = values[2];
        ret.push(SourceMapItem {
            source: SourceItem { offset: values[0], length: values[1], file_name: l[file_index].clone() },
            jump: if let Some(jump_value) = jump_value {
                Some(match jump_value {
                    "i" => JumpType::FunctionIn,
                    "o" => JumpType::FunctionOut,
                    "-" => JumpType::Regular,
                    _ => return Err(Error::UnknownSourceMapJump),
                })
            } else {
                None
            },
        });
    }
    Ok(ret)
}
