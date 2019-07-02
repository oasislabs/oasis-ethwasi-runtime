use mantle::{Context, Service};

#[derive(Service)]
pub struct MantleCounter {
    count: u64,
}

impl MantleCounter {
    pub fn new(_ctx: &Context) -> Result<Self, String> {
        Ok(Self { count: 0 })
    }

    pub fn get_count(&mut self, _ctx: &Context) -> Result<u64, String> {
        Ok(self.count)
    }

    pub fn get_count2(&mut self, _ctx: &Context, a: u64, b: String) -> Result<Vec<u8>, String> {
        Ok("count".as_bytes().to_vec())
    }

    pub fn increment_count(&mut self, _ctx: &Context) -> Result<(), String> {
        self.count += 1;
        Ok(())
    }
}

fn main() {
    mantle::service!(MantleCounter);
}
