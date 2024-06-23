use rand::Rng;

#[derive(Copy, Clone, Debug)]
pub struct QueryPage {
    page_size: usize,
    current_offset: usize,
    seed: i32,
}

impl QueryPage {
    pub fn from_query_offset(
        query_offset: &str,
        page_size: usize,
    ) -> Result<Self, anyhow::Error> {
        let mut value = query_offset.split(';');
        let mut rng = rand::thread_rng();

        match (value.next(), value.next()) {
            (Some(offset), Some(seed)) => Ok(Self {
                page_size,
                current_offset: offset.parse()?,
                seed: seed.parse()?,
            }),
            _ => Ok(Self {
                page_size,
                current_offset: 0,
                seed: rng.gen(),
            }),
        }
    }

    pub fn next_query_offset(&self, current_result_len: usize) -> String {
        if current_result_len >= self.page_size {
            format!("{};{}", self.current_offset + self.page_size, self.seed)
        } else {
            String::new() // empty string means no more results
        }
    }

    pub fn skip(&self) -> usize {
        self.current_offset
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }
    
    pub fn seed(&self) -> i32 {
        self.seed
    }
}
