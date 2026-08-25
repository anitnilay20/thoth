pub mod duck_db;

use crate::error::Result;
use duckdb::arrow::array::RecordBatch;

pub trait FileLoader {
    fn query(&self, query: &str) -> Result<Vec<RecordBatch>>;
    fn fetch(
        &self,
        filters: Vec<String>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<RecordBatch>>;
    fn size(&self) -> Result<u128>;
    fn len(&self) -> Result<usize>;
    fn get(&self, index: usize) -> Result<RecordBatch>;
    fn open(&self, path: &str, alias: &str) -> Result<()>;
}
