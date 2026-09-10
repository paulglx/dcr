#[derive(Default)]
pub struct Search {
    pub active: bool,
    pub query: String,
}

impl Search {
    /// Closes the prompt and forgets the query.
    pub fn reset(&mut self) {
        self.active = false;
        self.query.clear();
    }
}
