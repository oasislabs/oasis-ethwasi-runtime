use mantle::{Context, Service};

#[derive(Service)]
pub struct MantleCounter {
    count: u64,
}

impl MantleCounter {
    pub fn new(_ctx: &Context) -> Result<Self, String> {
        Ok(Self {
            count: 0
        })
    }
}

fn main() {
    mantle::service!(MantleCounter);
}
