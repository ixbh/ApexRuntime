//! Plain tables (TSV or CSV) and the two operations the Spreadsheet Method
//! builds on: **preflight** (is every sheet cell filled?) and **overlap**
//! (join two traces on a key and classify every value).

use std::collections::BTreeMap;
use std::path::Path;

/// A loaded table: header names plus rows of string cells.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Parse TSV if the header contains a tab, otherwise CSV (no quoting:
    /// our traces never contain commas inside cells).
    pub fn parse(text: &str) -> Self {
        let mut lines = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
        let Some(header) = lines.next() else {
            return Self::default();
        };
        let sep = if header.contains('\t') { '\t' } else { ',' };
        let split = |l: &str| {
            l.split(sep)
                .map(|c| c.trim().to_owned())
                .collect::<Vec<_>>()
        };
        Self {
            headers: split(header),
            rows: lines.map(split).collect(),
        }
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(path)?))
    }

    pub fn col(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|h| h == name)
    }

    pub fn cell<'a>(&'a self, row: &'a [String], name: &str) -> Option<&'a str> {
        self.col(name).and_then(|i| row.get(i)).map(String::as_str)
    }
}

/// How one compared value turned out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    Matching,
    Divergent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Divergence {
    pub key: String,
    pub column: String,
    pub reference: f64,
    pub port: f64,
}

/// Result of overlapping a reference trace with a port trace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TraceReport {
    /// Per compared column: (matching, divergent) counts.
    pub per_column: BTreeMap<String, (usize, usize)>,
    /// Divergences in reference row order. The first one is the bug to fix.
    pub divergent: Vec<Divergence>,
    /// Keys present in the reference only (unimplemented behaviour).
    pub missing: Vec<String>,
    /// Keys present in the port only (spurious behaviour).
    pub extra: Vec<String>,
}

impl TraceReport {
    pub fn first_divergence(&self) -> Option<&Divergence> {
        self.divergent.first()
    }

    pub fn is_clean(&self) -> bool {
        self.divergent.is_empty() && self.missing.is_empty() && self.extra.is_empty()
    }
}

/// One compared column: reference name, port name, absolute tolerance.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnSpec {
    pub reference: String,
    pub port: String,
    pub tolerance: f64,
}

/// Join `reference` and `port` on their key columns and compare `columns`.
/// Non-numeric cells compare as exact strings.
pub fn overlap(
    reference: &Table,
    ref_key: &str,
    port: &Table,
    port_key: &str,
    columns: &[ColumnSpec],
) -> Result<TraceReport, String> {
    let rk = reference
        .col(ref_key)
        .ok_or(format!("reference has no column {ref_key:?}"))?;
    let pk = port
        .col(port_key)
        .ok_or(format!("port has no column {port_key:?}"))?;
    let mut cols = Vec::new();
    for c in columns {
        let ri = reference
            .col(&c.reference)
            .ok_or(format!("reference has no column {:?}", c.reference))?;
        let pi = port
            .col(&c.port)
            .ok_or(format!("port has no column {:?}", c.port))?;
        cols.push((c, ri, pi));
    }

    let port_rows: BTreeMap<&str, &Vec<String>> = port
        .rows
        .iter()
        .filter_map(|r| r.get(pk).map(|k| (k.as_str(), r)))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut report = TraceReport::default();

    for row in &reference.rows {
        let Some(key) = row.get(rk) else { continue };
        seen.insert(key.as_str());
        let Some(prow) = port_rows.get(key.as_str()) else {
            report.missing.push(key.clone());
            continue;
        };
        for (spec, ri, pi) in &cols {
            let (a, b) = (
                row.get(*ri).map_or("", String::as_str),
                prow.get(*pi).map_or("", String::as_str),
            );
            let (class, av, bv) = match (a.parse::<f64>(), b.parse::<f64>()) {
                (Ok(x), Ok(y)) => {
                    let ok = (x - y).abs() <= spec.tolerance;
                    (
                        if ok {
                            Class::Matching
                        } else {
                            Class::Divergent
                        },
                        x,
                        y,
                    )
                }
                _ => (
                    if a == b {
                        Class::Matching
                    } else {
                        Class::Divergent
                    },
                    f64::NAN,
                    f64::NAN,
                ),
            };
            let entry = report.per_column.entry(spec.reference.clone()).or_default();
            match class {
                Class::Matching => entry.0 += 1,
                Class::Divergent => {
                    entry.1 += 1;
                    report.divergent.push(Divergence {
                        key: key.clone(),
                        column: spec.reference.clone(),
                        reference: av,
                        port: bv,
                    });
                }
            }
        }
    }
    report.extra = port_rows
        .keys()
        .filter(|k| !seen.contains(*k))
        .map(|k| (*k).to_owned())
        .collect();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tsv_and_csv() {
        let t = Table::parse("a\tb\n1\t2\n");
        assert_eq!(t.headers, ["a", "b"]);
        let c = Table::parse("# comment\na,b\n1,2\n3,4\n");
        assert_eq!(c.rows.len(), 2);
        assert_eq!(c.cell(&c.rows[1], "b"), Some("4"));
    }

    #[test]
    fn overlap_classifies_matching_divergent_missing_extra() {
        let r = Table::parse("tick,x\n1,0.0\n2,1.0\n3,2.0\n4,3.0\n");
        let p = Table::parse("cmd,px\n1,0.0\n2,1.005\n3,2.5\n5,9.0\n");
        let spec = [ColumnSpec {
            reference: "x".into(),
            port: "px".into(),
            tolerance: 0.01,
        }];
        let rep = overlap(&r, "tick", &p, "cmd", &spec).unwrap();
        assert_eq!(rep.per_column["x"], (2, 1));
        assert_eq!(rep.first_divergence().unwrap().key, "3");
        assert_eq!(rep.missing, ["4"]);
        assert_eq!(rep.extra, ["5"]);
        assert!(!rep.is_clean());
    }
}
