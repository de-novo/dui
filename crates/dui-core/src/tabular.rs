use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CsvMock {
    pub sample: String,
    #[serde(default)]
    headers: bool,
    #[serde(default)]
    label_columns: BTreeSet<usize>,
    #[serde(default)]
    value_columns: BTreeSet<usize>,
}

impl CsvMock {
    /// Keep row keys and grouping fields. Remap labels consistently and rotate
    /// measure tuples together so their original units and ranges are preserved.
    pub fn generate(&self, sample: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(sample);
        let rows = reader.records().collect::<Result<Vec<_>, _>>()?;
        let (head, data) = rows
            .split_at_checked(usize::from(self.headers))
            .ok_or("missing CSV header")?;
        let first = data.first().ok_or("empty CSV sample")?;
        let columns = self.label_columns.union(&self.value_columns);
        if !self.label_columns.is_disjoint(&self.value_columns)
            || (self.label_columns.is_empty() && self.value_columns.is_empty())
            || columns.into_iter().any(|column| *column >= first.len())
        {
            return Err("CSV mock requires distinct, in-range label or value columns".into());
        }
        let mut writer = csv::Writer::from_writer(Vec::new());
        for record in head {
            writer.write_record(record)?;
        }
        let mut labels = BTreeMap::new();
        for (index, row) in data.iter().enumerate() {
            let donor = &data[(index + 1) % data.len()];
            writer.write_record(row.iter().enumerate().map(|(column, cell)| {
                if self.label_columns.contains(&column) && !cell.is_empty() {
                    let next = labels.len() + 1;
                    format!("Sample {}", labels.entry((column, cell)).or_insert(next))
                } else if self.value_columns.contains(&column) {
                    donor[column].to_owned()
                } else {
                    cell.to_owned()
                }
            }))?;
        }
        Ok(String::from_utf8(writer.into_inner()?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_group_keys_and_csv_quoting_when_generating_new_rows() {
        let spec: CsvMock = serde_json::from_value(
            serde_json::json!({"sample":"x","headers":true,"labelColumns":[1],"valueColumns":[2]}),
        )
        .unwrap();
        let input = b"group,label,amount\r\n\"a,b\",first,12\r\na,second,5\r\nb,first,8\r\n";
        assert_eq!(
            spec.generate(input).unwrap(),
            "group,label,amount\n\"a,b\",Sample 1,5\na,Sample 2,8\nb,Sample 1,12\n"
        );
        assert!(spec.generate(b"group\na\n").is_err());
    }
}
