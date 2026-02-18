use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use stream_datastore::{DataStore, Stream};

#[derive(Clone)]
pub struct MockDataStore {
    pub existing_ids: HashSet<String>,
    pub inserted: Arc<Mutex<Vec<Stream>>>,
    pub fail_with: Option<String>,
}

impl Default for MockDataStore {
    fn default() -> Self {
        Self {
            existing_ids: HashSet::new(),
            inserted: Arc::new(Mutex::new(Vec::new())),
            fail_with: None,
        }
    }
}

impl MockDataStore {
    pub fn failing(msg: &str) -> Self {
        Self {
            fail_with: Some(msg.to_string()),
            ..Default::default()
        }
    }
}

impl DataStore for MockDataStore {
    async fn get_existing_stream_ids(
        &self,
        _video_ids: &[&str],
    ) -> anyhow::Result<HashSet<String>> {
        Ok(self.existing_ids.clone())
    }

    async fn insert_stream(&self, stream: &Stream) -> anyhow::Result<()> {
        if let Some(ref msg) = self.fail_with {
            return Err(anyhow::anyhow!("{}", msg));
        }
        self.inserted.lock().unwrap().push(stream.clone());
        Ok(())
    }
}
