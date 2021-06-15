use crate::{error::Error, session::{self, DeviceType}};

pub struct Device {

}

impl Device {
    pub fn speak(&self) {
        todo!()
    }
}

pub struct Session {

}

impl Session {
    pub fn new() -> Result<Self, Error> {
        todo!()
    }

    pub fn default_device(&self, _device_type: DeviceType) -> Result<session::Device, Error> {
        todo!()
    }
}
