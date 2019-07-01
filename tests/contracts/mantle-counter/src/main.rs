#[macro_use]
extern crate serde;

use mantle::{Address, Context, Event, Service};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Eq, PartialEq, failure::Fail)]
pub enum Error {
    #[fail(display = "Unknown error occured.")]
    Unknown,
}

#[derive(Service, Default)]
pub struct MantleCounter {
    count: u64,
}

impl MantleCounter {

    pub fn new(ctx: &Context) -> Result<Self> {
        Ok(Self {
            count: 0
        })
    }

    pub fn get_count(&mut self, ctx: &Context) -> Result<u64> {
        Ok(self.count)
    }

    pub fn increment_count(&mut self, ctx: &Context) -> Result<()> {
        Ok(())
    }
}


fn main() {
    mantle::service!(MantleCounter);
}
