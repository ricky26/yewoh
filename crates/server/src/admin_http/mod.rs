use actix_web::dev::{AppService, HttpServiceFactory};

pub struct AdminHttpApi {

}

impl AdminHttpApi {
    pub fn new() -> AdminHttpApi {
        Self {}
    }
}

impl HttpServiceFactory for AdminHttpApi {
    fn register(self, _config: &mut AppService) {

    }
}