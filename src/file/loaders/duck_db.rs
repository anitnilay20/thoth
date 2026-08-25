use crate::{error::Result, file::loaders::FileLoader};
use duckdb::{Connection, arrow::array::RecordBatch};

pub struct DuckdbConnection {
    conn: Connection,
}

impl DuckdbConnection {
    pub fn new() -> Result<Self> {
        Ok(Self {
            conn: Connection::open_in_memory().unwrap(),
        })
    }
}

// fn value_to_string(v: ValueRef<'_>) -> String {
//     match v {
//         ValueRef::Null => "NULL".to_string(),
//         ValueRef::Boolean(b) => b.to_string(),
//         ValueRef::TinyInt(n) => n.to_string(),
//         ValueRef::SmallInt(n) => n.to_string(),
//         ValueRef::Int(n) => n.to_string(),
//         ValueRef::BigInt(n) => n.to_string(),
//         ValueRef::HugeInt(n) => n.to_string(),
//         ValueRef::UTinyInt(n) => n.to_string(),
//         ValueRef::USmallInt(n) => n.to_string(),
//         ValueRef::UInt(n) => n.to_string(),
//         ValueRef::UBigInt(n) => n.to_string(),
//         ValueRef::Float(n) => n.to_string(),
//         ValueRef::Double(n) => n.to_string(),
//         ValueRef::Decimal(d) => d.to_string(),
//         ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
//         ValueRef::Blob(bytes) => format!("<{} bytes>", bytes.len()),
//         ValueRef::Timestamp(_, n) => n.to_string(),
//         ValueRef::Date32(n) => n.to_string(),
//         ValueRef::Time64(_, n) => n.to_string(),
//         other => format!("{other:?}"), // fallback for any type not explicitly handled
//     }
// }

impl FileLoader for DuckdbConnection {
    fn open(&self, path: &str, alias: &str) -> Result<()> {
        self.conn.execute(
            &format!("CREATE VIEW {alias} AS select * from \"{path}\""),
            [],
        )?;
        Ok(())
    }

    fn query(&self, query: &str) -> Result<Vec<RecordBatch>> {
        let mut stmt = self.conn.prepare(query).unwrap();
        let rows: Vec<_> = stmt.stream_arrow([]).unwrap().collect();
        Ok(rows)
    }

    fn fetch(
        &self,
        filters: Vec<String>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<RecordBatch>> {
        // let mut stmt = self
        //     .conn
        //     .prepare(format!("SELECT * FROM '{}'", "ola").as_str())
        //     .unwrap();
        // stmt.execute([]).unwrap();
        // for i in 0..stmt.column_count() {
        //     println!(
        //         "{}: {:?}",
        //         stmt.column_name(i).unwrap(),
        //         stmt.column_type(i)
        //     );
        // }

        // Ok(())
        todo!()
    }

    fn size(&self) -> Result<u128> {
        todo!()
    }

    fn len(&self) -> Result<usize> {
        todo!()
    }

    fn get(&self, index: usize) -> Result<RecordBatch> {
        todo!()
    }
}
